use crate::domain::{
    models::{api::*, error::Error},
    services::memories::{Filters, MemoryService, normalize, tag_name},
};
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
fn text(m: &Value, title: bool) -> String {
    let keys = if title {
        vec!["content", "summary", "title"]
    } else {
        vec!["content", "summary"]
    };
    keys.into_iter()
        .filter_map(|k| m[k].as_str())
        .find(|s| !s.is_empty())
        .unwrap_or("")
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
pub fn candidates(
    memories: &[Value],
    entities: &HashMap<String, HashSet<String>>,
    relationships: &HashMap<String, Vec<Value>>,
) -> Value {
    let mut duplicates = vec![];
    let mut seen = HashMap::new();
    for m in memories {
        let content = text(m, true);
        if content.is_empty() {
            continue;
        }
        let key = (m["memory_type"].as_str().unwrap(), content);
        if let Some(existing) = seen.get(&key) {
            duplicates.push(json!({"memory_a_id":existing,"memory_b_id":m["id"],"reason":"identical normalized content and memory_type"}));
        } else {
            seen.insert(key, m["id"].as_str().unwrap());
        }
    }
    let eligible: Vec<_> = memories
        .iter()
        .filter(|m| {
            ["decision", "preference", "instruction"].contains(&m["memory_type"].as_str().unwrap())
        })
        .collect();
    let mut contradictions = vec![];
    for (i, a) in eligible.iter().enumerate() {
        for b in eligible.iter().skip(i + 1) {
            let aid = a["id"].as_str().unwrap();
            let bid = b["id"].as_str().unwrap();
            let shared = entities
                .get(aid)
                .zip(entities.get(bid))
                .is_some_and(|(a, b)| !a.is_disjoint(b));
            if !shared {
                continue;
            }
            let ta = text(a, false);
            let tb = text(b, false);
            if ta.is_empty() || tb.is_empty() || ta == tb {
                continue;
            }
            let explained = relationships.get(aid).is_some_and(|rels| {
                rels.iter().any(|r| {
                    let other = if r["source_memory_id"] == aid {
                        r["target_memory_id"].as_str()
                    } else {
                        r["source_memory_id"].as_str()
                    };
                    other == Some(bid)
                        && [
                            "supersedes",
                            "corrects",
                            "contradicts",
                            "related_to",
                            "derived_from",
                        ]
                        .contains(&r["relation_type"].as_str().unwrap())
                })
            });
            if explained {
                continue;
            }
            contradictions.push(json!({"memory_a_id":aid,"memory_b_id":bid,"reason":format!("shared entity, same {} type, differing content (uncertain conflict \u{2014} not auto-resolved)",a["memory_type"].as_str().unwrap())}));
        }
    }
    json!({"candidate_duplicates":duplicates,"candidate_contradictions":contradictions,"applied_changes":[]})
}
impl MemoryService {
    pub async fn consolidate(&self, mut request: ConsolidationRequest) -> Result<Value, Error> {
        let max = crate::core::config::Config::default().consolidation_max_candidates;
        if request.memory_ids.is_empty() && request.filters.is_none() {
            return Err(Error::invalid(
                "consolidation requires memory_ids or filters (never the whole corpus)",
            ));
        }
        let mut memories = vec![];
        if !request.memory_ids.is_empty() {
            for id in request.memory_ids.iter().take(max) {
                match self.get(id, &Filters::new(), false).await {
                    Ok(m) => memories.push(m),
                    Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => {}
                    Err(e) => return Err(e),
                }
            }
        } else if let Some(filters) = request.filters.as_mut() {
            filters.limit = if filters.limit == 0 {
                max as i64
            } else {
                filters.limit.min(max as i64)
            };
            filters.workspace_id = filters.workspace_id.as_ref().map(|s| normalize(s));
            filters.tags = filters
                .tags
                .iter()
                .map(|t| {
                    let (a, b) = tag_name(t);
                    format!("{a}:{b}")
                })
                .collect();
            memories = self.store.consolidation_candidates(filters).await?;
        }
        let mut entities = HashMap::new();
        let mut relationships = HashMap::new();
        for memory in &memories {
            let id = memory["id"].as_str().unwrap();
            entities.insert(
                id.into(),
                self.store
                    .entity_links(id)
                    .await?
                    .into_iter()
                    .map(|e| e["entity_id"].as_str().unwrap().to_owned())
                    .collect(),
            );
            relationships.insert(id.into(), self.store.relationships(id).await?);
        }
        let mut report = candidates(&memories, &entities, &relationships);
        if request.mode == ConsolidationMode::ApplySafe {
            let by_id: HashMap<_, _> = memories
                .iter()
                .map(|m| (m["id"].as_str().unwrap(), m))
                .collect();
            let duplicates = report["candidate_duplicates"].as_array().unwrap().clone();
            for duplicate in duplicates {
                let a = by_id[duplicate["memory_a_id"].as_str().unwrap()];
                let b = by_id[duplicate["memory_b_id"].as_str().unwrap()];
                let date = |m: &Value| {
                    chrono::DateTime::parse_from_rfc3339(m["created_at"].as_str().unwrap())
                        .expect("stored timestamp")
                };
                let (older, newer) = if date(a) <= date(b) { (a, b) } else { (b, a) };
                if newer["status"] != "active" {
                    continue;
                }
                let new_id = newer["id"].as_str().unwrap();
                let old_id = older["id"].as_str().unwrap();
                self.store
                    .ensure_relationship(
                        new_id,
                        &RelationshipCreateRequest {
                            target_memory_id: old_id.into(),
                            relation_type: "related_to".into(),
                            source: Some("consolidation".into()),
                            confidence: None,
                        },
                    )
                    .await?;
                self.store
                    .update_memory(new_id, &json!({"status":"archived"}))
                    .await?;
                self.store
                    .event(
                        new_id,
                        "consolidated",
                        json!({"duplicate_of":old_id}),
                        Some("hippocampus_consolidator"),
                    )
                    .await?;
                report["applied_changes"].as_array_mut().unwrap().push(json!({"action":"archived_duplicate","memory_id":new_id,"detail":format!("duplicate_of={old_id}")}));
            }
        }
        Ok(report)
    }
}
