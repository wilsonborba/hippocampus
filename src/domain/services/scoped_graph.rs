use crate::{
    core::config::Config,
    dal::local::store::{Store, Table},
    domain::{
        models::{error::Error, search::SearchRequest},
        services::memories::{Filters, projection},
    },
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
fn request(filters: &Filters) -> Result<SearchRequest, Error> {
    serde_json::from_value(
        json!({"workspace_id":filters.get("workspace_id"),"app_id":filters.get("app_id")}),
    )
    .map_err(|_| Error::invalid("workspace_id is required"))
    .map(|mut r: SearchRequest| {
        r.scope.user_id = match filters.get("user_id") {
            None => crate::domain::models::scope::UserFilter::Any,
            Some(s) if s.is_empty() => crate::domain::models::scope::UserFilter::AppOnly,
            Some(s) => crate::domain::models::scope::UserFilter::User(s.clone()),
        };
        r
    })
}
fn number(filters: &Filters, key: &str, default: usize) -> Result<usize, Error> {
    filters
        .get(key)
        .map(|s| {
            s.parse()
                .map_err(|_| Error::invalid(format!("{key} must be nonnegative")))
        })
        .unwrap_or(Ok(default))
}
struct Scoped {
    memories: BTreeMap<String, Value>,
    tags: Vec<Value>,
    entities: Vec<Value>,
    files: Vec<Value>,
    relationships: Vec<Value>,
}
impl Scoped {
    async fn load(store: &Store, filters: &Filters) -> Result<Self, Error> {
        let r = request(filters)?;
        let memories: BTreeMap<_, _> = store
            .search_candidates(&r, None)
            .await?
            .into_iter()
            .map(|m| (m["id"].as_str().unwrap().to_owned(), m))
            .collect();
        let ids: Vec<_> = memories.keys().cloned().collect();
        Ok(Self {
            memories,
            tags: store.graph_links(&ids, Table::Tag).await?,
            entities: store.graph_links(&ids, Table::Entity).await?,
            files: store
                .graph_links(&ids, Table::Resource)
                .await?
                .into_iter()
                .filter(|r| r["resource_type"] == "file")
                .collect(),
            relationships: store.graph_links(&ids, Table::Relationship).await?,
        })
    }
    fn node_links(&self, id: &str) -> BTreeSet<String> {
        let (kind, key) = id.split_once(':').unwrap_or(("memory", id));
        if kind == "memory" {
            return if self.memories.contains_key(key) {
                BTreeSet::from([key.to_owned()])
            } else {
                BTreeSet::new()
            };
        }
        let links = match kind {
            "tag" => &self.tags,
            "entity" => &self.entities,
            "resource" => &self.files,
            _ => return BTreeSet::new(),
        };
        links
            .iter()
            .filter(|v| v["id"] == key)
            .map(|v| v["memory_id"].as_str().unwrap().to_owned())
            .collect()
    }
}
fn node(id: &str, kind: &str, label: Value, metadata: Value) -> Value {
    json!({"id":id,"node_type":kind,"label":label,"metadata":metadata})
}
fn edge(a: Value, b: Value, kind: Value) -> Value {
    json!({"source_id":a,"target_id":b,"edge_type":kind})
}
pub async fn graph(store: &Store, filters: &Filters, root: Option<&str>) -> Result<Value, Error> {
    let config = Config::default();
    let limit = number(filters, "limit", config.graph_default_max_nodes)?;
    let depth = number(filters, "depth", config.graph_max_depth)?;
    if limit == 0 || depth > config.scope_graph_max_depth {
        return Err(Error::invalid(format!(
            "limit must be positive and depth must be 0 to {}",
            config.scope_graph_max_depth
        )));
    }
    let scoped = Scoped::load(store, filters).await?;
    let seeds = match root {
        Some(id) => {
            let ids = scoped.node_links(id);
            if ids.is_empty() {
                return Err(Error::not_found("memory", id));
            }
            ids
        }
        None => scoped.memories.keys().cloned().collect(),
    };
    let mut selected: BTreeSet<_> = seeds.iter().take(limit).cloned().collect();
    let mut truncated = seeds.len() > selected.len();
    let mut frontier = selected.clone();
    for _ in 0..depth {
        let mut next = BTreeSet::new();
        for rel in &scoped.relationships {
            let a = rel["source_memory_id"].as_str().unwrap();
            let b = rel["target_memory_id"].as_str().unwrap();
            for (a, b) in [(a, b), (b, a)] {
                if frontier.contains(a) && !selected.contains(b) {
                    if selected.len() + next.len() < limit {
                        next.insert(b.to_owned());
                    } else {
                        truncated = true;
                    }
                }
            }
        }
        selected.extend(next.iter().cloned());
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    let mut nodes = BTreeMap::new();
    let mut edges = vec![];
    for id in &selected {
        let m = &scoped.memories[id];
        let label = m["title"].as_str().or(m["summary"].as_str()).unwrap_or(id);
        nodes.insert(
            id.clone(),
            node(id, "memory", json!(label), projection(m, Table::Memory)),
        );
    }
    for (kind, table, links) in [
        ("tag", Table::Tag, &scoped.tags),
        ("entity", Table::Entity, &scoped.entities),
        ("resource", Table::Resource, &scoped.files),
    ] {
        for link in links {
            if !selected.contains(link["memory_id"].as_str().unwrap()) {
                continue;
            }
            let id = format!("{kind}:{}", link["id"].as_str().unwrap());
            let label = if kind == "resource" {
                link["title"]
                    .as_str()
                    .unwrap_or(link["id"].as_str().unwrap())
            } else {
                link["canonical_name"].as_str().unwrap()
            };
            let node_kind = if kind == "resource" { "file" } else { kind };
            nodes.insert(
                id.clone(),
                node(&id, node_kind, json!(label), projection(link, table)),
            );
            edges.push(edge(
                link["memory_id"].clone(),
                json!(id),
                if kind == "tag" {
                    json!("tagged_with")
                } else if kind == "entity" {
                    json!("mentions")
                } else {
                    link["relationship"].clone()
                },
            ));
        }
    }
    for rel in &scoped.relationships {
        if selected.contains(rel["source_memory_id"].as_str().unwrap())
            && selected.contains(rel["target_memory_id"].as_str().unwrap())
        {
            edges.push(edge(
                rel["source_memory_id"].clone(),
                rel["target_memory_id"].clone(),
                rel["relation_type"].clone(),
            ));
        }
    }
    edges.sort_by_key(Value::to_string);
    edges.dedup();
    let context = root.and_then(|id| nodes.get(id)).cloned();
    let mut out = json!({"nodes":nodes.into_values().collect::<Vec<_>>(),"edges":edges,"truncated":truncated});
    if root.is_some() {
        out["node"] = context.unwrap_or(Value::Null);
    }
    Ok(out)
}
pub async fn names(store: &Store, filters: &Filters, kind: Table) -> Result<Value, Error> {
    let limit = number(
        filters,
        "limit",
        Config::default().list_default_limit as usize,
    )?;
    let r = request(filters)?;
    let needle = filters.get("q").map(String::as_str).unwrap_or_default();
    Ok(Value::Array(
        store
            .scoped_names(&r, kind, needle, limit)
            .await?
            .iter()
            .map(|v| projection(v, kind))
            .collect(),
    ))
}
