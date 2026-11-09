use super::{
    memories::scope_filter,
    store::{Store, id},
};
use crate::domain::{
    models::{
        error::Error,
        personas::{PersonaCreate, PersonaUpdate},
        scope::{ReadScope, UserFilter, WriteScope},
    },
    services::personas::normalize,
};
use serde_json::{Value, json};
use sqlx::{Postgres, QueryBuilder};
fn failure(e: sqlx::Error) -> Error {
    if e.as_database_error()
        .is_some_and(|d| d.is_unique_violation())
    {
        Error::new(
            axum::http::StatusCode::CONFLICT,
            "entity_conflict",
            "The scoped name or is_self persona already exists",
        )
    } else {
        e.into()
    }
}
pub fn read_scope(s: &WriteScope) -> ReadScope {
    ReadScope {
        workspace_id: crate::domain::services::memories::normalize(&s.workspace_id),
        app_id: Some(s.app_id.clone()),
        user_id: s
            .user_id
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| UserFilter::User(s.clone()))
            .unwrap_or(UserFilter::AppOnly),
    }
}
impl Store {
    pub async fn personas(&self, scope: &ReadScope) -> Result<Vec<Value>, Error> {
        let mut q = QueryBuilder::<Postgres>::new("SELECT row_to_json(e) FROM entity e WHERE ");
        scope_filter(&mut q, scope);
        q.push(
            " AND entity_type IN ('person','bot') AND status='active' ORDER BY canonical_name,id",
        );
        let rows: Vec<Value> = q.build_query_scalar().fetch_all(&self.pool).await?;
        let mut out = vec![];
        for mut row in rows {
            let aliases: Vec<String> = sqlx::query_scalar(
                "SELECT alias FROM entity_alias WHERE entity_id=$1 ORDER BY normalized_alias",
            )
            .bind(row["id"].as_str().unwrap())
            .fetch_all(&self.pool)
            .await?;
            row["aliases"] = json!(aliases);
            out.push(row);
        }
        Ok(out)
    }
    pub async fn create_persona(&self, r: &PersonaCreate) -> Result<Value, Error> {
        let id = id();
        let mut tx = self.pool.begin().await?;
        let user = r.scope.user_id.as_deref().filter(|s| !s.is_empty());
        sqlx::query("INSERT INTO entity (id,entity_type,canonical_name,normalized_name,status,metadata_json,created_at,updated_at,workspace_id,app_id,user_id,is_self) VALUES ($1,$2,$3,$4,'active','{}',now(),now(),$5,$6,$7,$8)").bind(&id).bind(&r.entity_type).bind(r.canonical_name.trim()).bind(normalize(&r.canonical_name)).bind(crate::domain::services::memories::normalize(&r.scope.workspace_id)).bind(&r.scope.app_id).bind(user).bind(r.is_self).execute(&mut *tx).await.map_err(failure)?;
        for alias in &r.aliases {
            sqlx::query("INSERT INTO entity_alias(id,entity_id,alias,normalized_alias,created_at) VALUES ($1,$2,$3,$4,now()) ON CONFLICT(entity_id,normalized_alias) DO NOTHING").bind(super::store::id()).bind(&id).bind(alias.trim()).bind(normalize(alias)).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.personas(&read_scope(&r.scope))
            .await?
            .into_iter()
            .find(|v| v["id"] == id)
            .ok_or_else(|| Error::not_found("entity", &id))
    }
    pub async fn update_persona(&self, id: &str, r: &PersonaUpdate) -> Result<Value, Error> {
        let mut tx = self.pool.begin().await?;
        let current: Option<Value> =
            sqlx::query_scalar("SELECT row_to_json(e) FROM entity e WHERE id=$1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        let current = current.ok_or_else(|| Error::not_found("entity", id))?;
        if current["workspace_id"]
            != crate::domain::services::memories::normalize(&r.scope.workspace_id)
            || current["app_id"] != r.scope.app_id
            || current["user_id"].as_str().filter(|s| !s.is_empty())
                != r.scope.user_id.as_deref().filter(|s| !s.is_empty())
            || !matches!(current["entity_type"].as_str(), Some("person" | "bot"))
        {
            return Err(Error::not_found("entity", id));
        }
        let name = r
            .canonical_name
            .as_deref()
            .unwrap_or(current["canonical_name"].as_str().unwrap());
        let is_self = r.is_self.unwrap_or(current["is_self"].as_bool().unwrap());
        sqlx::query("UPDATE entity SET canonical_name=$1,normalized_name=$2,is_self=$3,updated_at=now() WHERE id=$4").bind(name.trim()).bind(normalize(name)).bind(is_self).bind(id).execute(&mut *tx).await.map_err(failure)?;
        if let Some(aliases) = &r.aliases {
            sqlx::query("DELETE FROM entity_alias WHERE entity_id=$1")
                .bind(id)
                .execute(&mut *tx)
                .await?;
            for alias in aliases {
                sqlx::query("INSERT INTO entity_alias(id,entity_id,alias,normalized_alias,created_at) VALUES ($1,$2,$3,$4,now()) ON CONFLICT(entity_id,normalized_alias) DO NOTHING").bind(super::store::id()).bind(id).bind(alias.trim()).bind(normalize(alias)).execute(&mut *tx).await?;
            }
        }
        tx.commit().await?;
        self.personas(&read_scope(&r.scope))
            .await?
            .into_iter()
            .find(|v| v["id"] == id)
            .ok_or_else(|| Error::not_found("entity", id))
    }
}
