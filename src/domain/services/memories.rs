use crate::{
    dal::local::store::{Store, Table, now},
    domain::models::{api::*, error::Error},
};
use axum::http::StatusCode;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashSet};

pub type Filters = BTreeMap<String, String>;
const TYPES: &[&str] = &[
    "working",
    "episodic",
    "semantic",
    "decision",
    "preference",
    "observation",
    "instruction",
    "correction",
    "artifact",
    "file",
    "derived",
];
const RELATIONS: &[&str] = &[
    "related_to",
    "derived_from",
    "supports",
    "contradicts",
    "supersedes",
    "corrects",
    "references",
    "belongs_to",
    "discusses",
    "implements",
    "depends_on",
    "caused_by",
    "resulted_in",
    "part_of",
    "associated_with",
];
pub fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}
pub fn tag_name(value: &str) -> (String, String) {
    let (a, b) = value.split_once(':').unwrap_or(("general", value));
    (normalize(a), normalize(b))
}
fn maxlen(field: &str, value: Option<&str>, max: usize) -> Result<(), Error> {
    if let Some(value) = value
        && value.chars().count() > max
    {
        return Err(Error::invalid(format!(
            "{field} must be {max} characters or fewer; got {}",
            value.chars().count()
        )));
    }
    Ok(())
}
pub fn projection(value: &Value, table: Table) -> Value {
    let fields: &[&str] = match table {
        Table::Memory => &[
            "id",
            "workspace_id",
            "app_id",
            "user_id",
            "memory_type",
            "status",
            "title",
            "summary",
            "content",
            "importance",
            "confidence",
            "created_at",
            "updated_at",
            "observed_at",
            "valid_from",
            "valid_until",
            "expires_at",
            "last_accessed_at",
            "access_count",
            "reinforcement_count",
        ],
        Table::Tag => &["id", "namespace", "value", "canonical_name", "created_at"],
        Table::Entity => &[
            "id",
            "entity_type",
            "canonical_name",
            "description",
            "created_at",
        ],
        Table::Resource => &[
            "id",
            "resource_type",
            "source_system",
            "external_id",
            "uri",
            "title",
            "ownership",
            "status",
            "content_type",
            "checksum",
            "size_bytes",
            "created_at",
        ],
        Table::Relationship => &[
            "id",
            "source_memory_id",
            "target_memory_id",
            "relation_type",
            "confidence",
            "source",
            "created_at",
        ],
        Table::Event => &[
            "id",
            "memory_id",
            "event_type",
            "actor_type",
            "actor_id",
            "payload",
            "created_at",
        ],
        _ => return value.clone(),
    };
    Value::Object(
        fields
            .iter()
            .map(|k| (k.to_string(), value[*k].clone()))
            .collect(),
    )
}
#[derive(Clone)]
pub struct MemoryService {
    pub store: Store,
}
impl MemoryService {
    pub async fn get(&self, id: &str, filters: &Filters, touch: bool) -> Result<Value, Error> {
        let memory = self
            .store
            .get(Table::Memory, id)
            .await?
            .ok_or_else(|| Error::not_found("memory", id))?;
        if memory["status"] == "deleted"
            || filters.iter().any(|(key, value)| match key.as_str() {
                "workspace_id" => memory[key] != normalize(value),
                "app_id" => memory[key] != *value,
                "user_id" => {
                    if value.is_empty() {
                        !memory[key].is_null()
                    } else {
                        memory[key] != *value
                    }
                }
                _ => false,
            })
        {
            return Err(Error::not_found("memory", id));
        }
        if touch {
            self.store.update_memory(id,&json!({"access_count":memory["access_count"].as_i64().unwrap_or(0)+1,"last_accessed_at":now()})).await?;
        }
        // v1 returns the row read before the access update.
        Ok(memory)
    }
    pub async fn create(&self, body: MemoryCreateRequest) -> Result<Value, Error> {
        let kind = body.memory_type.as_deref().unwrap_or("semantic");
        if !TYPES.contains(&kind) {
            return Err(Error::invalid(format!("unknown memory_type: '{kind}'")));
        }
        if [&body.title, &body.summary, &body.content]
            .iter()
            .all(|s| s.as_ref().is_none_or(|s| s.is_empty()))
        {
            return Err(Error::invalid(
                "at least one of content, summary, or title is required",
            ));
        }
        let workspace = normalize(&body.workspace_id);
        if workspace.is_empty() {
            return Err(Error::invalid("workspace_id cannot be blank"));
        }
        maxlen("workspace_id", Some(&workspace), 64)?;
        if body.app_id.trim().is_empty() {
            return Err(Error::invalid("app_id cannot be blank"));
        }
        maxlen("app_id", Some(&body.app_id), 64)?;
        for (name, value) in [
            ("observed_at", &body.observed_at),
            ("valid_from", &body.valid_from),
            ("valid_until", &body.valid_until),
            ("expires_at", &body.expires_at),
        ] {
            if let Some(s) = value
                && chrono::DateTime::parse_from_rfc3339(s).is_err()
            {
                return Err(Error::invalid(format!("{name} must be a valid datetime")));
            }
        }
        for entity in &body.entities {
            maxlen(
                "entities[].role",
                entity.get("role").and_then(Value::as_str),
                32,
            )?;
            maxlen(
                "entities[].source",
                entity.get("source").and_then(Value::as_str),
                32,
            )?;
        }
        for resource in &body.resources {
            maxlen(
                "resources[].relationship",
                resource.get("relationship").and_then(Value::as_str),
                32,
            )?;
            maxlen(
                "resources[].source",
                resource.get("source").and_then(Value::as_str),
                32,
            )?;
        }
        let mut value = serde_json::to_value(&body).expect("memory serialization");
        value["workspace_id"] = json!(workspace);
        value["memory_type"] = json!(kind);
        value["status"] = json!("active");
        value["user_id"] = json!(body.user_id.as_ref().filter(|s| !s.is_empty()));
        value["access_count"] = json!(0);
        value["reinforcement_count"] = json!(0);
        value["metadata_json"] = json!(body.metadata);
        let memory = self.store.insert(Table::Memory, value).await?;
        let id = memory["id"].as_str().expect("memory id");
        for tag in body.tags {
            self.add_tag(id, &tag, "user", None).await?;
        }
        for entity in body.entities {
            let req: EntityAssociateRequest = serde_json::from_value(json!(entity))
                .map_err(|_| Error::invalid("The entity reference is not valid"))?;
            self.associate_entity(id, req, Some("user")).await?;
        }
        for resource in body.resources {
            let req: ResourceRegisterRequest = serde_json::from_value(json!(resource))
                .map_err(|_| Error::invalid("The resource reference is not valid"))?;
            let relationship = resource
                .get("relationship")
                .and_then(Value::as_str)
                .unwrap_or("references");
            let resource = self.register_resource(req).await?;
            self.link_resource(id, &resource, relationship, Some("user"))
                .await?;
        }
        self.store
            .event(id, "created", json!({}), Some("user"))
            .await?;
        Ok(memory)
    }
    pub async fn list(&self, filters: &Filters) -> Result<Vec<Value>, Error> {
        let mut normalized = filters.clone();
        if let Some(workspace) = normalized.get_mut("workspace_id") {
            *workspace = normalize(workspace);
        }
        if let Some(tag) = normalized.get_mut("tag") {
            let (a, b) = tag_name(tag);
            *tag = format!("{a}:{b}");
        }
        self.store.list_memories(&normalized).await
    }

    pub async fn update(&self, id: &str, body: MemoryUpdateRequest) -> Result<Value, Error> {
        self.get(id, &Filters::new(), false).await?;
        let mut fields = serde_json::to_value(body).expect("update serialization");
        fields
            .as_object_mut()
            .expect("object")
            .retain(|_, v| !v.is_null());
        if let Some(metadata) = fields.as_object_mut().expect("object").remove("metadata") {
            fields["metadata_json"] = metadata;
        }
        self.store.update_memory(id, &fields).await?;
        let names: Vec<_> = [
            "title",
            "summary",
            "importance",
            "confidence",
            "valid_from",
            "valid_until",
            "expires_at",
            "metadata_json",
        ]
        .into_iter()
        .filter(|key| fields.get(*key).is_some())
        .collect();
        self.store
            .event(id, "updated", json!({"fields":names}), None)
            .await?;
        self.get(id, &Filters::new(), false).await
    }
    pub async fn add_tag(
        &self,
        id: &str,
        canonical: &str,
        source: &str,
        confidence: Option<f64>,
    ) -> Result<Value, Error> {
        maxlen("source", Some(source), 32)?;
        let (a, b) = tag_name(canonical);
        let canonical = format!("{a}:{b}");
        self.store
            .ensure_tag(id, &canonical, source, confidence)
            .await
    }

    pub async fn tag(&self, id: &str, body: TagRequest) -> Result<Vec<Value>, Error> {
        self.get(id, &Filters::new(), false).await?;
        let mut tags = vec![];
        for tag in body.tags {
            tags.push(self.add_tag(id, &tag, &body.source, None).await?);
        }
        self.store
            .event(
                id,
                "tagged",
                json!({"tags":tags.iter().map(|v|v["canonical_name"].clone()).collect::<Vec<_>>()}),
                None,
            )
            .await?;
        Ok(tags)
    }
    pub async fn untag(&self, id: &str, tag: &str) -> Result<(), Error> {
        if self.store.remove_tag(id, tag).await? {
            self.store
                .event(id, "untagged", json!({"tag_id":tag}), None)
                .await?;
        }
        Ok(())
    }
    pub async fn associate_entity(
        &self,
        id: &str,
        body: EntityAssociateRequest,
        actor: Option<&str>,
    ) -> Result<Value, Error> {
        self.get(id, &Filters::new(), false).await?;
        maxlen("role", body.role.as_deref(), 32)?;
        maxlen("source", body.source.as_deref(), 32)?;
        let normalized = body
            .canonical_name
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        self.store.ensure_entity(id, body, actor, &normalized).await
    }
    pub async fn register_resource(&self, body: ResourceRegisterRequest) -> Result<Value, Error> {
        self.store.register_resource(body).await
    }

    pub async fn link_resource(
        &self,
        id: &str,
        resource: &Value,
        relationship: &str,
        source: Option<&str>,
    ) -> Result<(), Error> {
        maxlen("relationship", Some(relationship), 32)?;
        self.store
            .link_resource(id, resource, relationship, source)
            .await
    }

    pub async fn associate_resource(
        &self,
        id: &str,
        body: ResourceLinkRequest,
    ) -> Result<Value, Error> {
        self.get(id, &Filters::new(), false).await?;
        let resource = self
            .register_resource(ResourceRegisterRequest {
                resource_type: body.resource_type,
                source_system: body.source_system,
                external_id: body.external_id,
                uri: body.uri,
                title: None,
                content_type: None,
                checksum: None,
                size_bytes: None,
            })
            .await?;
        self.link_resource(id, &resource, &body.relationship, None)
            .await?;
        Ok(resource)
    }
    pub async fn relationship(
        &self,
        id: &str,
        body: RelationshipCreateRequest,
        event: bool,
    ) -> Result<Value, Error> {
        if !RELATIONS.contains(&body.relation_type.as_str()) {
            return Err(Error::invalid(format!(
                "unknown relation_type: '{}'",
                body.relation_type
            )));
        }
        self.get(id, &Filters::new(), false).await?;
        self.get(&body.target_memory_id, &Filters::new(), false)
            .await?;
        if id == body.target_memory_id {
            return Err(Error::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_relationship",
                "a memory cannot relate to itself",
            ));
        }
        let value = self.store.ensure_relationship(id, &body).await?;
        if event {
            self.store
                .event(
                    id,
                    "linked",
                    json!({"target":body.target_memory_id,"relation_type":body.relation_type}),
                    None,
                )
                .await?;
        }
        Ok(value)
    }
    pub async fn neighbors(
        &self,
        id: &str,
        kind: Option<&str>,
        depth: usize,
    ) -> Result<Vec<Value>, Error> {
        self.get(id, &Filters::new(), false).await?;
        let mut seen = HashSet::from([id.to_owned()]);
        let mut frontier = vec![id.to_owned()];
        let mut rows = vec![];
        for _ in 0..depth.clamp(1, crate::core::config::Config::default().graph_max_depth) {
            let mut next = vec![];
            for id in frontier {
                let rels = self.store.relationships(&id).await?;
                for rel in rels {
                    if kind.is_some_and(|k| rel["relation_type"] != k) {
                        continue;
                    }
                    if rows.len() >= 50 {
                        return Ok(rows);
                    }
                    let other = if rel["source_memory_id"] == id {
                        rel["target_memory_id"].as_str()
                    } else {
                        rel["source_memory_id"].as_str()
                    }
                    .expect("relationship endpoint")
                    .to_owned();
                    rows.push(rel);
                    if seen.insert(other.clone()) {
                        next.push(other);
                    }
                }
            }
            frontier = next;
            if frontier.is_empty() {
                break;
            }
        }
        Ok(rows)
    }
    pub async fn reinforce(&self, id: &str, body: ReinforceRequest) -> Result<Value, Error> {
        let memory = self.get(id, &Filters::new(), false).await?;
        self.store.update_memory(id,&json!({"reinforcement_count":memory["reinforcement_count"].as_i64().unwrap_or(0)+1,"last_reinforced_at":now()})).await?;
        self.store
            .event(
                id,
                "reinforced",
                json!({"source":body.source,"reason":body.reason}),
                None,
            )
            .await?;
        self.get(id, &Filters::new(), false).await
    }
    pub async fn replace(&self, id: &str, new: &str, correct: bool) -> Result<(), Error> {
        if id == new {
            return Err(Error::new(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_relationship",
                if correct {
                    "a memory cannot correct itself"
                } else {
                    "a memory cannot supersede itself"
                },
            ));
        }
        self.relationship(
            new,
            RelationshipCreateRequest {
                target_memory_id: id.to_owned(),
                relation_type: if correct { "corrects" } else { "supersedes" }.into(),
                source: None,
                confidence: None,
            },
            false,
        )
        .await?;
        self.store
            .update_memory(id, &json!({"status":"superseded"}))
            .await?;
        self.store
            .event(
                id,
                if correct { "corrected" } else { "superseded" },
                json!({"by":new}),
                None,
            )
            .await
    }
    pub async fn status(
        &self,
        id: &str,
        status: &str,
        payload: Value,
        filters: &Filters,
    ) -> Result<Value, Error> {
        if status == "active" {
            let memory = self
                .store
                .get(Table::Memory, id)
                .await?
                .ok_or_else(|| Error::not_found("memory", id))?;
            if memory["status"] == "deleted" {
                return Err(Error::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_memory_status",
                    "hard-deleted memories cannot be restored",
                ));
            }
        } else {
            self.get(id, filters, false).await?;
        }
        self.store
            .update_memory(id, &json!({"status":status}))
            .await?;
        self.store
            .event(
                id,
                match status {
                    "active" => "restored",
                    "archived" => "archived",
                    _ => "forgotten",
                },
                payload,
                None,
            )
            .await?;
        self.get(id, &Filters::new(), false).await
    }
}
