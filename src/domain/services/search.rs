use crate::{
    core::config::Config,
    dal::local::store::{Store, Table},
    domain::models::{embedding::EmbeddingSource, error::Error, search::SearchRequest},
    domain::services::memories::projection,
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};
pub fn rrf(lists: &[(&str, Vec<String>)], k: f64) -> Vec<(String, f64, Vec<String>)> {
    let mut scores: BTreeMap<String, (f64, Vec<String>)> = BTreeMap::new();
    for (reason, list) in lists {
        for (i, id) in list.iter().enumerate() {
            let v = scores.entry(id.clone()).or_default();
            v.0 += 1.0 / (k + i as f64 + 1.0);
            v.1.push((*reason).into());
        }
    }
    let mut out: Vec<_> = scores
        .into_iter()
        .map(|(id, (score, reasons))| (id, score, reasons))
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}
pub async fn search(
    store: &Store,
    model: &dyn EmbeddingSource,
    r: SearchRequest,
) -> Result<Value, Error> {
    let config = Config::default();
    if r.expand > 3 || r.limit == Some(0) {
        return Err(Error::invalid(
            "expand must be 0 to 3 and limit must be positive",
        ));
    }
    let words = store.search_words(&r).await?;
    let mut lists = vec![("word", words)];
    if model.name() != "none"
        && let Some(text) = r.text.as_ref().filter(|s| !s.trim().is_empty())
        && let Some(vector) = model.embed(text).await
        && !vector.is_empty()
        && vector.iter().all(|v| v.is_finite())
    {
        lists.push((
            "semantic",
            store.search_vectors(&r, model.name(), &vector).await?,
        ));
    }
    let ranked = rrf(&lists, config.search_rrf_k);
    let mut selected: Vec<_> = ranked
        .into_iter()
        .take(r.limit.unwrap_or(config.list_default_limit as usize))
        .collect();
    let mut seen: HashSet<_> = selected.iter().map(|x| x.0.clone()).collect();
    let selected_ids: Vec<_> = selected.iter().map(|x| x.0.clone()).collect();
    let candidates: BTreeMap<_, _> = store
        .search_candidates(
            &r,
            if r.expand == 0 {
                Some(&selected_ids)
            } else {
                None
            },
        )
        .await?
        .into_iter()
        .map(|v| (v["id"].as_str().unwrap().to_owned(), v))
        .collect();
    let mut frontier: Vec<_> = selected.iter().map(|x| x.0.clone()).collect();
    for _ in 0..r.expand {
        let mut next = vec![];
        for id in frontier {
            for link in store.relationships(&id).await? {
                for key in ["source_memory_id", "target_memory_id"] {
                    if let Some(id) = link[key].as_str()
                        && candidates.contains_key(id)
                        && seen.insert(id.to_owned())
                    {
                        next.push(id.to_owned());
                    }
                }
            }
        }
        next.sort();
        for id in &next {
            selected.push((id.clone(), 0.0, vec!["relationship".into()]));
        }
        frontier = next;
    }
    Ok(Value::Array(selected.into_iter().filter_map(|(id,score,reasons)|candidates.get(&id).map(|m|json!({"memory":projection(m,Table::Memory),"score":score,"match_reasons":reasons}))).collect()))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reciprocal_rank_fusion() {
        let r = rrf(
            &[
                ("word", vec!["a".into(), "b".into()]),
                ("semantic", vec!["b".into(), "c".into()]),
            ],
            60.0,
        );
        assert_eq!(r[0].0, "b");
        assert_eq!(r[0].2, vec!["word", "semantic"]);
        assert!((r[0].1 - (1.0 / 62.0 + 1.0 / 61.0)).abs() < 1e-12);
    }
}
