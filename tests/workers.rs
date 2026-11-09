use hippocampus::{
    dal::{
        local::{store::Store, test_database},
        remote::documents::Documents,
    },
    domain::{models::api::MemoryCreateRequest, services::memories::MemoryService, tasks::workers},
};
use serde_json::json;
#[tokio::test]
#[ignore = "requires the isolated hippocampus_v2_test database"]
async fn workers_with_fixed_dates() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let workspace = format!("workers-{}", uuid::Uuid::new_v4().simple());
    let request:MemoryCreateRequest=serde_json::from_value(json!({"app_id":"fixture","workspace_id":workspace,"title":"Worker fixture","content":"Snapshot text","expires_at":"2029-01-01T00:00:00Z"})).unwrap();
    let memory = service.create(request).await.unwrap();
    let id = memory["id"].as_str().unwrap();
    sqlx::query("UPDATE memory SET created_at='9998-01-01T00:00:00Z' WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    let clock = chrono::DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let expired = workers::expire(&store, 1, clock).await.unwrap();
    assert_eq!(expired.expired_memory_ids, vec![id]);
    let docs = Documents::default();
    let skipped = workers::sync_documents(&store, &docs, 1).await.unwrap();
    assert_eq!(skipped.skipped_no_document, 1);
    store
        .update_memory(
            id,
            &json!({"document_id":format!("memory:{id}"),"document_store":"couchdb"}),
        )
        .await
        .unwrap();
    let synced = workers::sync_documents(&store, &docs, 1).await.unwrap();
    assert_eq!(synced.processed, 1);
    assert_eq!(synced.synced, 1);
    assert_eq!(docs.get(id).await.unwrap()["content"], "Snapshot text");
    let reconciled = workers::reconcile(&store, &docs, 1).await.unwrap();
    assert!(reconciled.missing_document.is_empty());
    assert_eq!(reconciled.missing_embedding, vec![id]);
    let missing = workers::reconcile(&store, &Documents::default(), 1)
        .await
        .unwrap();
    assert_eq!(missing.missing_document, vec![id]);
    sqlx::query("DELETE FROM memory_event WHERE memory_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM memory WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}
