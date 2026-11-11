use hippocampus::{
    dal::local::{store::Store, test_database},
    domain::{models::api::MemoryCreateRequest, services::memories::MemoryService},
};
use serde_json::json;
#[tokio::test]
#[ignore = "requires hippocampus_v2_test"]
async fn uuid_cleanup_rolls_back_in_a_test_transaction() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let service = MemoryService {
        store: Store { pool: pool.clone() },
    };
    let uuid_workspace = uuid::Uuid::new_v4().to_string();
    let named = format!("keep-{}", uuid::Uuid::new_v4().simple());
    let mut ids = vec![];
    for workspace in [&uuid_workspace, &named] {
        let request: MemoryCreateRequest = serde_json::from_value(
            json!({"app_id":"legacy","workspace_id":workspace,"title":"Migration fixture"}),
        )
        .unwrap();
        let m = service.create(request).await.unwrap();
        ids.push(m["id"].as_str().unwrap().to_owned());
    }
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(include_str!(
        "../migrations/0002_remove_v1_cortex_memories.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let removed: bool = sqlx::query_scalar("SELECT NOT EXISTS(SELECT 1 FROM memory WHERE id=$1)")
        .bind(&ids[0])
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    let kept: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM memory WHERE id=$1)")
        .bind(&ids[1])
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert!(removed && kept);
    tx.rollback().await.unwrap();
    for id in ids {
        sqlx::query("DELETE FROM memory_event WHERE memory_id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM memory WHERE id=$1")
            .bind(&id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
