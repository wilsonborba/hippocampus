use super::store::Store;
use crate::domain::models::{deletion::DeleteRequest, error::Error};
use crate::domain::services::memories::normalize;
use serde_json::Value;
use sqlx::{Postgres, Transaction};
pub struct MemoryDeletion {
    tx: Transaction<'static, Postgres>,
    pub ids: Vec<String>,
    pub documents: Vec<String>,
    pub files: Vec<Value>,
}
impl Store {
    pub async fn begin_memory_delete(&self, r: &DeleteRequest) -> Result<MemoryDeletion, Error> {
        let mut tx = self.pool.begin().await?;
        let memories: Vec<Value> = sqlx::query_scalar(
            "SELECT row_to_json(m) FROM memory m WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(&r.memory_ids)
        .fetch_all(&mut *tx)
        .await?;
        let user = r.scope.user_id.as_deref().filter(|s| !s.is_empty());
        if memories.iter().any(|m| {
            m["workspace_id"] != normalize(&r.scope.workspace_id)
                || m["app_id"] != r.scope.app_id
                || m["user_id"].as_str().filter(|s| !s.is_empty()) != user
        }) {
            return Err(Error::new(
                axum::http::StatusCode::FORBIDDEN,
                "scope_mismatch",
                "A memory is outside the delete scope",
            ));
        }
        let ids: Vec<String> = memories
            .iter()
            .map(|m| m["id"].as_str().unwrap().to_owned())
            .collect();
        let documents = memories
            .iter()
            .filter(|m| m["document_store"] == "couchdb")
            .map(|m| m["id"].as_str().unwrap().to_owned())
            .collect();
        let files:Vec<Value>=sqlx::query_scalar("SELECT row_to_json(r) FROM resource r WHERE resource_type='file' AND id IN (SELECT resource_id FROM memory_resource WHERE memory_id=ANY($1)) ORDER BY id FOR UPDATE").bind(&ids).fetch_all(&mut *tx).await?;
        let tags: Vec<String> =
            sqlx::query_scalar("SELECT tag_id FROM memory_tag WHERE memory_id=ANY($1)")
                .bind(&ids)
                .fetch_all(&mut *tx)
                .await?;
        sqlx::query("DELETE FROM memory_relationship WHERE source_memory_id=ANY($1) OR target_memory_id=ANY($1)").bind(&ids).execute(&mut *tx).await?;
        for table in [
            "memory_embedding",
            "memory_entity",
            "memory_event",
            "memory_resource",
            "memory_tag",
        ] {
            sqlx::query(&format!("DELETE FROM {table} WHERE memory_id=ANY($1)"))
                .bind(&ids)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM memory WHERE id=ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM tag WHERE id=ANY($1) AND NOT EXISTS (SELECT 1 FROM memory_tag WHERE tag_id=tag.id)").bind(tags).execute(&mut *tx).await?;
        let mut unused = vec![];
        for file in files {
            let id = file["id"].as_str().unwrap();
            let affected=sqlx::query("DELETE FROM resource WHERE id=$1 AND NOT EXISTS(SELECT 1 FROM memory_resource WHERE resource_id=$1)").bind(id).execute(&mut *tx).await?.rows_affected();
            if affected > 0 {
                unused.push(file);
            }
        }
        Ok(MemoryDeletion {
            tx,
            ids,
            documents,
            files: unused,
        })
    }
}
impl MemoryDeletion {
    pub async fn commit(self) -> Result<Vec<String>, Error> {
        self.tx.commit().await?;
        Ok(self.ids)
    }
}
