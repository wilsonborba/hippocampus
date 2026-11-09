use crate::domain::models::error::Error;
use serde_json::{Value, json};
use sqlx::{PgPool, Postgres, QueryBuilder, types::Json};
#[derive(Clone)]
pub struct Store {
    pub pool: PgPool,
}
pub fn id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}
pub fn now() -> Value {
    json!(chrono::Utc::now().to_rfc3339())
}
#[derive(Clone, Copy)]
pub enum Table {
    Memory,
    Tag,
    MemoryTag,
    Entity,
    MemoryEntity,
    Resource,
    MemoryResource,
    Relationship,
    Event,
}
impl Table {
    pub fn name(self) -> &'static str {
        match self {
            Self::Memory => "memory",
            Self::Tag => "tag",
            Self::MemoryTag => "memory_tag",
            Self::Entity => "entity",
            Self::MemoryEntity => "memory_entity",
            Self::Resource => "resource",
            Self::MemoryResource => "memory_resource",
            Self::Relationship => "memory_relationship",
            Self::Event => "memory_event",
        }
    }
}
impl Store {
    pub async fn get(&self, table: Table, id: &str) -> Result<Option<Value>, Error> {
        Ok(sqlx::query_scalar::<_, Json<Value>>(&format!(
            "SELECT row_to_json(t) FROM {} t WHERE id=$1",
            table.name()
        ))
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .map(|x| x.0))
    }
    pub async fn insert(&self, table: Table, mut value: Value) -> Result<Value, Error> {
        value["id"] = json!(id());
        value["created_at"] = now();
        value["updated_at"] = now();
        let sql = format!(
            "INSERT INTO {} SELECT * FROM json_populate_record(NULL::{},$1::json) RETURNING row_to_json({}.*)",
            table.name(),
            table.name(),
            table.name()
        );
        Ok(sqlx::query_scalar::<_, Json<Value>>(&sql)
            .bind(Json(value))
            .fetch_one(&self.pool)
            .await?
            .0)
    }
    pub async fn update_memory(&self, id: &str, fields: &Value) -> Result<(), Error> {
        if fields.as_object().is_none_or(|m| m.is_empty()) {
            return Ok(());
        }
        let mut query = QueryBuilder::<Postgres>::new("UPDATE memory SET ");
        let mut separated = query.separated(",");
        for (key, val) in fields.as_object().expect("fields object") {
            // This set is internal. HTTP callers cannot select SQL identifiers.
            let allowed = [
                "title",
                "summary",
                "importance",
                "confidence",
                "valid_from",
                "valid_until",
                "expires_at",
                "metadata_json",
                "status",
                "last_accessed_at",
                "access_count",
                "reinforcement_count",
                "last_reinforced_at",
                "document_store",
                "document_id",
            ];
            if !allowed.contains(&key.as_str()) {
                return Err(Error::invalid("The field cannot be updated directly"));
            }
            separated.push(format!(
                "{key} = (SELECT {key} FROM json_populate_record(NULL::memory,"
            ));
            separated.push_bind_unseparated(Json(json!({key:val})));
            separated.push_unseparated("::json))");
        }
        separated.push("updated_at = now()");
        query.push(" WHERE id=").push_bind(id);
        query.push(" AND (");
        let mut changes = query.separated(" OR ");
        for (key, value) in fields.as_object().expect("fields") {
            changes.push(format!(
                "{key} IS DISTINCT FROM (SELECT {key} FROM json_populate_record(NULL::memory,"
            ));
            changes
                .push_bind_unseparated(Json(json!({key:value})))
                .push_unseparated("::json))");
        }
        query.push(")");
        query.build().execute(&self.pool).await?;
        Ok(())
    }
    pub async fn values(&self, sql: &str, binds: &[String]) -> Result<Vec<Value>, Error> {
        let mut q = sqlx::query_scalar::<_, Json<Value>>(sql);
        for b in binds {
            q = q.bind(b);
        }
        Ok(q.fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|x| x.0)
            .collect())
    }
    pub async fn event(
        &self,
        memory: &str,
        kind: &str,
        payload: Value,
        actor: Option<&str>,
    ) -> Result<(), Error> {
        self.insert(
            Table::Event,
            json!({"memory_id":memory,"event_type":kind,"actor_type":actor,"payload":payload}),
        )
        .await?;
        Ok(())
    }
}
impl Store {
    pub async fn relationships(&self, id: &str) -> Result<Vec<Value>, Error> {
        self.values("SELECT row_to_json(r) FROM memory_relationship r WHERE source_memory_id=$1 OR target_memory_id=$1",&[id.into()]).await
    }
    pub async fn memory_tags(&self, id: &str) -> Result<Vec<Value>, Error> {
        self.values("SELECT row_to_json(t) FROM tag t JOIN memory_tag mt ON mt.tag_id=t.id WHERE mt.memory_id=$1",&[id.into()]).await
    }
    pub async fn entity_links(&self, id: &str) -> Result<Vec<Value>, Error> {
        self.values(
            "SELECT row_to_json(t) FROM memory_entity t WHERE memory_id=$1",
            &[id.into()],
        )
        .await
    }
    pub async fn resource_links(&self, id: &str) -> Result<Vec<Value>, Error> {
        self.values(
            "SELECT row_to_json(t) FROM memory_resource t WHERE memory_id=$1",
            &[id.into()],
        )
        .await
    }
}

impl Store {
    pub async fn list_memories(
        &self,
        filters: &std::collections::BTreeMap<String, String>,
    ) -> Result<Vec<Value>, Error> {
        let mut q = QueryBuilder::<Postgres>::new("SELECT row_to_json(m) FROM memory m WHERE 1=1");
        if !filters.contains_key("status") {
            q.push(" AND status NOT IN ('deleted','forgotten')");
        }
        for key in ["workspace_id", "app_id", "memory_type", "status"] {
            if let Some(value) = filters.get(key) {
                let value = value.clone();
                q.push(format!(" AND {key} = ")).push_bind(value);
            }
        }
        if let Some(user) = filters.get("user_id") {
            if user.is_empty() {
                q.push(" AND user_id IS NULL");
            } else {
                q.push(" AND user_id = ").push_bind(user);
            }
        }
        if let Some(text) = filters.get("text").filter(|x| !x.is_empty()) {
            let pattern = format!("%{text}%");
            q.push(" AND (title LIKE ")
                .push_bind(pattern.clone())
                .push(" OR summary LIKE ")
                .push_bind(pattern.clone())
                .push(" OR content LIKE ")
                .push_bind(pattern)
                .push(")");
        }
        if let Some(tag) = filters.get("tag") {
            let (a, b) = tag.split_once(':').expect("canonical tag");
            q.push(" AND id IN (SELECT memory_id FROM memory_tag JOIN tag ON tag_id=tag.id WHERE canonical_name=").push_bind(format!("{a}:{b}")).push(")");
        }
        let limit = filters
            .get("limit")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or(crate::core::config::Config::default().list_default_limit);
        q.push(" ORDER BY created_at DESC LIMIT ").push_bind(limit);
        Ok(q.build_query_scalar::<Json<Value>>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|x| x.0)
            .collect())
    }
}

impl Store {
    pub async fn ensure_tag(
        &self,
        id: &str,
        canonical: &str,
        source: &str,
        confidence: Option<f64>,
    ) -> Result<Value, Error> {
        let (namespace, value) = canonical.split_once(':').expect("canonical tag");
        let name = format!("{namespace}:{value}");
        let rows = self
            .values(
                "SELECT row_to_json(t) FROM tag t WHERE canonical_name=$1",
                std::slice::from_ref(&name),
            )
            .await?;
        let tag = if let Some(t) = rows.first() {
            t.clone()
        } else {
            self.insert(Table::Tag,json!({"namespace":namespace,"value":value,"canonical_name":name,"status":"active"})).await?
        };
        sqlx::query("INSERT INTO memory_tag (id,memory_id,tag_id,source,confidence,weight,created_at) VALUES ($1,$2,$3,$4,$5,1,now()) ON CONFLICT (memory_id,tag_id) DO UPDATE SET source=EXCLUDED.source, confidence=EXCLUDED.confidence,weight=1")
            .bind(crate::dal::local::store::id()).bind(id).bind(tag["id"].as_str()).bind(source).bind(confidence).execute(&self.pool).await?;
        Ok(tag)
    }
}

impl Store {
    pub async fn register_resource(&self, body: ResourceRegisterRequest) -> Result<Value, Error> {
        let mut q = QueryBuilder::<Postgres>::new(
            "SELECT row_to_json(r) FROM resource r WHERE source_system=",
        );
        q.push_bind(&body.source_system);
        if let Some(external) = &body.external_id {
            q.push(" AND resource_type=")
                .push_bind(&body.resource_type)
                .push(" AND external_id=")
                .push_bind(external);
        } else if let Some(uri) = &body.uri {
            q.push(" AND uri=").push_bind(uri);
        } else {
            q.push(" AND false");
        }
        if let Some(r) = q
            .build_query_scalar::<Json<Value>>()
            .fetch_optional(&self.pool)
            .await?
        {
            return Ok(r.0);
        }
        let mut value = serde_json::to_value(body).expect("resource serialization");
        value["ownership"] = json!("external");
        value["status"] = json!("unknown");
        value["metadata_json"] = json!({});
        self.insert(Table::Resource, value).await
    }
}

impl Store {
    pub async fn link_resource(
        &self,
        id: &str,
        resource: &Value,
        relationship: &str,
        source: Option<&str>,
    ) -> Result<(), Error> {
        sqlx::query("INSERT INTO memory_resource (id,memory_id,resource_id,relationship,source,created_at) VALUES ($1,$2,$3,$4,$5,now()) ON CONFLICT (memory_id,resource_id,relationship) DO NOTHING")
            .bind(crate::dal::local::store::id()).bind(id).bind(resource["id"].as_str()).bind(relationship).bind(source).execute(&self.pool).await?;
        Ok(())
    }
}

use crate::domain::models::api::ResourceRegisterRequest;

impl Store {
    pub async fn ensure_entity(
        &self,
        id: &str,
        body: crate::domain::models::api::EntityAssociateRequest,
        actor: Option<&str>,
        normalized: &str,
    ) -> Result<Value, Error> {
        let rows=self.values("SELECT row_to_json(e) FROM entity e WHERE (entity_type=$1 AND normalized_name=$2) OR (entity_type=$1 AND id IN (SELECT entity_id FROM entity_alias WHERE normalized_alias=$2)) LIMIT 1",&[body.entity_type.clone(),normalized.to_owned()]).await?;
        let entity = if let Some(v) = rows.first() {
            v.clone()
        } else {
            self.insert(Table::Entity,json!({"entity_type":body.entity_type,"canonical_name":body.canonical_name,"normalized_name":normalized,"status":"active","metadata_json":{}})).await?
        };
        let entity_id = entity["id"].as_str().expect("entity id");
        let exists:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memory_entity WHERE memory_id=$1 AND entity_id=$2 AND role IS NOT DISTINCT FROM $3)").bind(id).bind(entity_id).bind(&body.role).fetch_one(&self.pool).await?;
        if !exists {
            self.insert(Table::MemoryEntity,json!({"memory_id":id,"entity_id":entity_id,"role":body.role,"source":actor.or(body.source.as_deref()),"confidence":body.confidence})).await?;
        }
        Ok(entity)
    }
}

impl Store {
    pub async fn ensure_relationship(
        &self,
        id: &str,
        body: &crate::domain::models::api::RelationshipCreateRequest,
    ) -> Result<Value, Error> {
        let existing=self.values("SELECT row_to_json(r) FROM memory_relationship r WHERE source_memory_id=$1 AND target_memory_id=$2 AND relation_type=$3",&[id.into(),body.target_memory_id.clone(),body.relation_type.clone()]).await?;
        let value = if let Some(v) = existing.first() {
            v.clone()
        } else {
            self.insert(Table::Relationship,json!({"source_memory_id":id,"target_memory_id":body.target_memory_id,"relation_type":body.relation_type,"source":body.source,"confidence":body.confidence,"metadata_json":{}})).await?
        };

        Ok(value)
    }
}

impl Store {
    pub async fn remove_tag(&self, id: &str, tag: &str) -> Result<bool, Error> {
        Ok(
            sqlx::query("DELETE FROM memory_tag WHERE memory_id=$1 AND tag_id=$2")
                .bind(id)
                .bind(tag)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }
    pub async fn events(&self, id: &str) -> Result<Vec<Value>, Error> {
        self.values(
            "SELECT row_to_json(e) FROM memory_event e WHERE memory_id=$1 ORDER BY created_at",
            &[id.into()],
        )
        .await
    }
}

impl Store {
    pub async fn available(&self) -> bool {
        sqlx::query("SELECT 1").execute(&self.pool).await.is_ok()
    }
}

impl Store {
    pub async fn embedded_ids(&self) -> Result<std::collections::HashSet<String>, Error> {
        Ok(sqlx::query_scalar("SELECT memory_id FROM memory_embedding")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .collect())
    }
}
impl Store {
    pub async fn consolidation_candidates(
        &self,
        filters: &crate::domain::models::api::ConsolidationFilters,
    ) -> Result<Vec<Value>, Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT row_to_json(m) FROM memory m WHERE status='active'",
        );
        if let Some(workspace) = &filters.workspace_id {
            query.push(" AND workspace_id=").push_bind(workspace);
        }
        if let Some(kind) = filters.memory_types.first() {
            query.push(" AND memory_type=").push_bind(kind);
        }
        if !filters.tags.is_empty() {
            query.push(" AND id IN (SELECT memory_id FROM memory_tag JOIN tag ON tag_id=tag.id WHERE canonical_name IN (");
            let mut separated = query.separated(",");
            for tag in &filters.tags {
                separated.push_bind(tag);
            }
            query.push("))");
        }
        if let Some(after) = &filters.created_after {
            query
                .push(" AND created_at >= ")
                .push_bind(after)
                .push("::timestamptz");
        }
        if let Some(before) = &filters.created_before {
            query
                .push(" AND created_at <= ")
                .push_bind(before)
                .push("::timestamptz");
        }
        query
            .push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(filters.limit);
        Ok(query
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|v| v.0)
            .collect())
    }
}
