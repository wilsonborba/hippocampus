use crate::{
    dal::local::store::{Store, id},
    domain::models::{
        embedding::{EmbeddingMemory, EmbeddingStore, Pending},
        error::Error,
    },
};
use chrono::{DateTime, Utc};
use sqlx::Row;
impl EmbeddingStore for Store {
    fn batch<'a>(
        &'a self,
        model: &'a str,
        after: &'a str,
        limit: i64,
    ) -> Pending<'a, Result<Vec<EmbeddingMemory>, Error>> {
        Box::pin(async move {
            let rows=sqlx::query("SELECT m.id,m.title,m.summary,m.content,e.source_text_hash FROM memory m LEFT JOIN LATERAL (SELECT source_text_hash FROM memory_embedding WHERE memory_id=m.id AND model=$1 AND model_version IS NULL ORDER BY created_at DESC LIMIT 1) e ON true WHERE m.id>$2 AND m.status != 'deleted' ORDER BY m.id LIMIT $3").bind(model).bind(after).bind(limit).fetch_all(&self.pool).await?;
            rows.into_iter()
                .map(|r| {
                    Ok(EmbeddingMemory {
                        id: r.try_get("id")?,
                        title: r.try_get("title")?,
                        summary: r.try_get("summary")?,
                        content: r.try_get("content")?,
                        current_hash: r.try_get("source_text_hash")?,
                    })
                })
                .collect()
        })
    }
    fn save<'a>(
        &'a self,
        memory: &'a str,
        model: &'a str,
        vector: &'a [f64],
        hash: &'a str,
        clock: DateTime<Utc>,
    ) -> Pending<'a, Result<(), Error>> {
        Box::pin(async move {
            let mut tx = self.pool.begin().await?;
            let existing:Option<String>=sqlx::query_scalar("SELECT id FROM memory_embedding WHERE memory_id=$1 AND model=$2 AND model_version IS NULL LIMIT 1 FOR UPDATE").bind(memory).bind(model).fetch_optional(&mut *tx).await?;
            let vector_text = serde_json::to_string(vector).expect("finite vector");
            if let Some(id) = existing {
                sqlx::query("UPDATE memory_embedding SET dimensions=$1,embedding=$2::vector,source_text_hash=$3 WHERE id=$4").bind(vector.len() as i32).bind(vector_text).bind(hash).bind(id).execute(&mut *tx).await?;
            } else {
                sqlx::query("INSERT INTO memory_embedding (id,memory_id,model,model_version,dimensions,embedding,source_text_hash,created_at) VALUES ($1,$2,$3,NULL,$4,$5::vector,$6,$7)").bind(id()).bind(memory).bind(model).bind(vector.len() as i32).bind(vector_text).bind(hash).bind(clock).execute(&mut *tx).await?;
            }
            tx.commit().await?;
            Ok(())
        })
    }
}
