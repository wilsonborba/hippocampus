use crate::{
    dal::local::store::Table,
    domain::{
        models::error::Error,
        services::memories::{Filters, MemoryService},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

impl MemoryService {
    pub async fn graph(&self, root: &str, filters: &Filters) -> Result<Value, Error> {
        let config = crate::core::config::Config::default();
        let depth = filters
            .get("depth")
            .and_then(|x| x.parse::<i64>().ok())
            .filter(|n| *n != 0)
            .unwrap_or(config.graph_max_depth as i64)
            .clamp(1, config.graph_max_depth as i64);
        let cap = filters
            .get("max_nodes")
            .and_then(|x| x.parse::<i64>().ok())
            .unwrap_or(config.graph_default_max_nodes as i64)
            .max(0) as usize;
        let kinds: Option<HashSet<&str>> = filters
            .get("relation_types")
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').collect());
        let mut memories =
            BTreeMap::from([(root.to_owned(), self.get(root, filters, false).await?)]);
        let mut seen = HashSet::from([root.to_owned()]);
        let mut seen_edges = HashSet::new();
        let mut frontier = vec![root.to_owned()];
        let mut edges = vec![];
        let mut truncated = false;
        for _ in 0..depth {
            let mut next = vec![];
            for mid in frontier {
                let rels = self.store.relationships(&mid).await?;
                for rel in rels {
                    let edge_id = rel["id"].as_str().expect("relationship id").to_owned();
                    let kind = rel["relation_type"].as_str().expect("relationship kind");
                    if seen_edges.contains(&edge_id)
                        || kinds.as_ref().is_some_and(|k| !k.contains(kind))
                    {
                        continue;
                    }
                    let other = if rel["source_memory_id"] == mid {
                        rel["target_memory_id"].as_str()
                    } else {
                        rel["source_memory_id"].as_str()
                    }
                    .expect("endpoint")
                    .to_owned();
                    let memory = match self.get(&other, filters, false).await {
                        Ok(m) => m,
                        Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => continue,
                        Err(e) => return Err(e),
                    };
                    seen_edges.insert(edge_id);
                    edges.push(json!({"source_id":rel["source_memory_id"],"target_id":rel["target_memory_id"],"edge_type":kind,"confidence":rel["confidence"]}));
                    if !seen.contains(&other) {
                        if seen.len() >= cap {
                            truncated = true;
                            continue;
                        }
                        seen.insert(other.clone());
                        memories.insert(other.clone(), memory);
                        next.push(other);
                    }
                }
                if truncated {
                    break;
                }
            }
            frontier = next;
            if frontier.is_empty() || truncated {
                break;
            }
        }
        edges.retain(|e| {
            memories.contains_key(e["source_id"].as_str().unwrap())
                && memories.contains_key(e["target_id"].as_str().unwrap())
        });
        let mut nodes = vec![];
        for (id, m) in &memories {
            let label = ["title", "summary", "content"]
                .iter()
                .filter_map(|k| m[*k].as_str())
                .find(|s| !s.is_empty())
                .unwrap_or(id)
                .trim();
            let label = if label.chars().count() > 60 {
                label.chars().take(59).collect::<String>() + "…"
            } else {
                label.to_owned()
            };
            let preview = m["content"]
                .as_str()
                .unwrap_or("")
                .trim()
                .chars()
                .take(200)
                .collect::<String>();
            nodes.push(node(id,"memory",&label,m["memory_type"].clone(),json!({"status":m["status"],"importance":m["importance"],"created_at":m["created_at"],"content_preview":if preview.is_empty(){Value::Null}else{json!(preview)}})));
        }
        let mut auxiliary = HashSet::new();
        for id in memories.keys() {
            if enabled(filters, "include_tags") {
                for tag in self.store.memory_tags(id).await? {
                    let nid = format!("tag:{}", tag["id"].as_str().unwrap());
                    if auxiliary.insert(nid.clone()) {
                        nodes.push(node(
                            &nid,
                            "tag",
                            tag["canonical_name"].as_str().unwrap(),
                            Value::Null,
                            json!({}),
                        ));
                    }
                    edges.push(edge(id, &nid, "tagged_with", Value::Null));
                }
            }
            if enabled(filters, "include_entities") {
                for link in self.store.entity_links(id).await? {
                    let eid = link["entity_id"].as_str().unwrap();
                    let nid = format!("entity:{eid}");
                    if auxiliary.insert(nid.clone())
                        && let Some(entity) = self.store.get(Table::Entity, eid).await?
                    {
                        nodes.push(node(
                            &nid,
                            "entity",
                            entity["canonical_name"].as_str().unwrap(),
                            entity["entity_type"].clone(),
                            json!({}),
                        ));
                    }
                    edges.push(edge(id, &nid, "mentions", link["confidence"].clone()));
                }
            }
            if enabled(filters, "include_resources") {
                for link in self.store.resource_links(id).await? {
                    let rid = link["resource_id"].as_str().unwrap();
                    let nid = format!("resource:{rid}");
                    if auxiliary.insert(nid.clone())
                        && let Some(resource) = self.store.get(Table::Resource, rid).await?
                    {
                        let label = ["title", "uri", "id"]
                            .iter()
                            .filter_map(|k| resource[*k].as_str())
                            .find(|s| !s.is_empty())
                            .unwrap_or(rid);
                        nodes.push(node(
                            &nid,
                            "resource",
                            label,
                            resource["resource_type"].clone(),
                            json!({}),
                        ));
                    }
                    edges.push(edge(
                        id,
                        &nid,
                        link["relationship"].as_str().unwrap(),
                        Value::Null,
                    ));
                }
            }
        }
        let graph = json!({"nodes":nodes,"edges":edges,"root_ids":[root],"truncated":truncated||nodes.len()>config.graph_cluster_threshold});
        Ok(cluster(graph))
    }
}
fn enabled(filters: &Filters, key: &str) -> bool {
    filters.get(key).is_none_or(|x| x != "false" && x != "0")
}
fn node(id: &str, kind: &str, label: &str, subtitle: Value, metadata: Value) -> Value {
    json!({"id":id,"node_type":kind,"label":label,"subtitle":subtitle,"metadata":metadata,"cluster_of":null})
}
fn edge(source: &str, target: &str, kind: &str, confidence: Value) -> Value {
    json!({"source_id":source,"target_id":target,"edge_type":kind,"confidence":confidence})
}
fn cluster(graph: Value) -> Value {
    let all = graph["nodes"].as_array().expect("nodes");
    if all.len() <= crate::core::config::Config::default().graph_cluster_threshold {
        return graph;
    }
    let edges = graph["edges"].as_array().expect("edges");
    let roots: HashSet<_> = graph["root_ids"]
        .as_array()
        .expect("roots")
        .iter()
        .map(|x| x.as_str().unwrap())
        .collect();
    let mut keep = roots.clone();
    for e in edges {
        let a = e["source_id"].as_str().unwrap();
        let b = e["target_id"].as_str().unwrap();
        if roots.contains(a) {
            keep.insert(b);
        }
        if roots.contains(b) {
            keep.insert(a);
        }
    }
    let mut nodes = vec![];
    let mut groups: BTreeMap<(String, Option<String>), Vec<String>> = BTreeMap::new();
    for n in all {
        let id = n["id"].as_str().unwrap();
        if keep.contains(id) {
            nodes.push(n.clone());
        } else {
            groups
                .entry((
                    n["node_type"].as_str().unwrap().into(),
                    n["subtitle"].as_str().map(str::to_owned),
                ))
                .or_default()
                .push(id.to_owned());
        }
    }
    let mut result_edges: Vec<_> = edges
        .iter()
        .filter(|e| {
            keep.contains(e["source_id"].as_str().unwrap())
                && keep.contains(e["target_id"].as_str().unwrap())
        })
        .cloned()
        .collect();
    for ((kind, subtitle), members) in groups {
        let id = format!("cluster:{kind}:{}", subtitle.as_deref().unwrap_or("misc"));
        let label = format!(
            "{} {kind}(s){}",
            members.len(),
            subtitle
                .as_ref()
                .map(|s| format!(" • {s}"))
                .unwrap_or_default()
        );
        let mut n = node(&id, "cluster", &label, json!(subtitle), json!({}));
        n["cluster_of"] = json!(members);
        nodes.push(n);
        for e in edges {
            let a = e["source_id"].as_str().unwrap();
            let b = e["target_id"].as_str().unwrap();
            if members.iter().any(|s| s == a) && keep.contains(b) {
                result_edges.push(edge(&id, b, "member_of_cluster", Value::Null));
            } else if members.iter().any(|s| s == b) && keep.contains(a) {
                result_edges.push(edge(a, &id, "member_of_cluster", Value::Null));
            }
        }
    }
    json!({"nodes":nodes,"edges":result_edges,"root_ids":graph["root_ids"],"truncated":true})
}
