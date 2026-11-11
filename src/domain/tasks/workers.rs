use crate::{
    dal::{local::store::Store, remote::documents::Documents},
    domain::{models::error::Error, services::memories::Filters},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ExpirationReport {
    pub expired_memory_ids: Vec<String>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DocumentSyncReport {
    pub processed: usize,
    pub synced: usize,
    pub skipped_no_document: usize,
    pub failed: usize,
}
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ReconciliationReport {
    pub missing_document: Vec<String>,
    pub missing_embedding: Vec<String>,
}

pub fn is_expired(memory: &Value, now: DateTime<Utc>) -> bool {
    memory["expires_at"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .is_some_and(|s| s <= now)
}
pub async fn expire(
    store: &Store,
    limit: i64,
    now: DateTime<Utc>,
) -> Result<ExpirationReport, Error> {
    let candidates = store
        .list_memories(&Filters::from([
            ("status".into(), "active".into()),
            ("limit".into(), limit.to_string()),
        ]))
        .await?;
    let mut report = ExpirationReport::default();
    for memory in candidates {
        if is_expired(&memory, now) {
            let id = memory["id"].as_str().expect("id");
            store
                .update_memory(id, &json!({"status":"expired"}))
                .await?;
            store
                .event(id, "updated", json!({"reason":"expired"}), None)
                .await?;
            report.expired_memory_ids.push(id.to_owned());
        }
    }
    Ok(report)
}
pub fn document_snapshot(
    memory: &Value,
    existing: Option<Value>,
    tags: Vec<Value>,
    entities: Vec<Value>,
) -> Value {
    let mut document = existing.unwrap_or_else(|| json!({}));
    if document.get("schema_version").is_none() {
        document["schema_version"] = json!(1);
    }
    document["memory_id"] = memory["id"].clone();
    document["content"] = memory["content"].clone();
    document["summary"] = memory["summary"].clone();
    document["tags"] = json!(
        tags.iter()
            .map(|t| t["canonical_name"].clone())
            .collect::<Vec<_>>()
    );
    document["entities"] = json!(
        entities
            .iter()
            .map(|e| json!({"id":e["entity_id"],"role":e["role"]}))
            .collect::<Vec<_>>()
    );
    document
}
pub async fn sync_documents(
    store: &Store,
    documents: &Documents,
    limit: i64,
) -> Result<DocumentSyncReport, Error> {
    let candidates = store
        .list_memories(&Filters::from([("limit".into(), limit.to_string())]))
        .await?;
    let mut report = DocumentSyncReport::default();
    for memory in candidates {
        report.processed += 1;
        if memory["document_id"].as_str().is_none_or(str::is_empty) {
            report.skipped_no_document += 1;
            continue;
        }
        let id = memory["id"].as_str().expect("id");
        let snapshot = document_snapshot(
            &memory,
            documents.get(id).await,
            store.memory_tags(id).await?,
            store.entity_links(id).await?,
        );
        if documents.put(id, snapshot).await {
            report.synced += 1;
        } else {
            report.failed += 1;
            tracing::warn!("Document sync failed");
        }
    }
    Ok(report)
}
pub async fn reconcile(
    store: &Store,
    documents: &Documents,
    limit: i64,
) -> Result<ReconciliationReport, Error> {
    let candidates = store
        .list_memories(&Filters::from([("limit".into(), limit.to_string())]))
        .await?;
    let embedded = store.embedded_ids().await?;
    let mut report = ReconciliationReport::default();
    for memory in candidates {
        let id = memory["id"].as_str().expect("id");
        if memory["document_id"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
            && documents.get(id).await.is_none()
        {
            report.missing_document.push(id.into());
        }
        if !embedded.contains(id) {
            report.missing_embedding.push(id.into());
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_clock_expiration() {
        let clock = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        assert!(is_expired(
            &json!({"expires_at":"2030-01-01T00:00:00Z"}),
            clock
        ));
        assert!(!is_expired(
            &json!({"expires_at":"2030-01-01T00:00:01Z"}),
            clock
        ));
        assert!(!is_expired(&json!({"expires_at":null}), clock));
    }
    #[test]
    fn document_snapshot_preserves_existing_fields() {
        let memory = json!({"id":"m","content":"text","summary":null});
        let doc = document_snapshot(
            &memory,
            Some(json!({"schema_version":2,"extra":true})),
            vec![json!({"canonical_name":"topic:rust"})],
            vec![json!({"entity_id":"e","role":"subject"})],
        );
        assert_eq!(
            doc,
            json!({"memory_id":"m","content":"text","summary":null,"schema_version":2,"extra":true,"tags":["topic:rust"],"entities":[{"id":"e","role":"subject"}]})
        );
    }
}
