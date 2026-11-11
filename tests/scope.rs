use hippocampus::domain::models::scope::{ReadScope, UserFilter, WriteScope};
use serde_json::json;
#[test]
fn scope_is_explicit() {
    assert!(serde_json::from_value::<WriteScope>(json!({"workspace_id":"asodya"})).is_err());
    assert!(
        serde_json::from_value::<WriteScope>(json!({"workspace_id":"asodya","app_id":" "}))
            .is_err()
    );
    assert!(serde_json::from_value::<ReadScope>(json!({"app_id":"cortex"})).is_err());
    let all: ReadScope = serde_json::from_value(json!({"workspace_id":"asodya"})).unwrap();
    assert_eq!(all.user_id, UserFilter::Any);
    let a2a: ReadScope =
        serde_json::from_value(json!({"workspace_id":"asodya","user_id":null})).unwrap();
    assert_eq!(a2a.user_id, UserFilter::AppOnly);
}
#[tokio::test]
#[ignore = "requires the restored hippocampus_v2_test database"]
async fn real_database_isolation() {
    dotenvy::dotenv().ok();
    let pool = hippocampus::dal::local::test_database::connect()
        .await
        .unwrap();
    sqlx::migrate!().run(&pool).await.unwrap();
    let workspace = format!("scope-test-{}", uuid::Uuid::new_v4().simple());
    let mut tx = pool.begin().await.unwrap();
    for (id, app, user) in [
        ("a", "cortex", None),
        ("b", "cortex", Some("u1")),
        ("c", "cortex", Some("u2")),
        ("d", "other", Some("u1")),
    ] {
        sqlx::query("INSERT INTO memory (id, workspace_id, app_id, user_id, memory_type, status, access_count, reinforcement_count, metadata_json, created_at, updated_at) VALUES ($1,$2,$3,$4,'decision','active',0,0,'{}',now(),now())")
            .bind(format!("{workspace}-{id}")).bind(&workspace).bind(app).bind(user).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    for (app, user, count) in [
        (Some("cortex"), json!(null), 1),
        (Some("cortex"), json!("u1"), 1),
        (None, json!("u1"), 2),
    ] {
        let scope: ReadScope =
            serde_json::from_value(json!({"workspace_id":workspace,"app_id":app,"user_id":user}))
                .unwrap();
        assert_eq!(
            hippocampus::dal::local::memories::scoped_memory_ids(&pool, &scope)
                .await
                .unwrap()
                .len(),
            count
        );
    }
    let all: ReadScope = serde_json::from_value(json!({"workspace_id":workspace})).unwrap();
    assert_eq!(
        hippocampus::dal::local::memories::scoped_memory_ids(&pool, &all)
            .await
            .unwrap()
            .len(),
        4
    );
    let result=sqlx::query("INSERT INTO memory (id, workspace_id, memory_type, status, access_count, reinforcement_count, metadata_json, created_at, updated_at) VALUES ($1,$2,'decision','active',0,0,'{}',now(),now())")
        .bind(format!("{workspace}-invalid")).bind(&workspace).execute(&pool).await;
    assert!(result.is_err());
    sqlx::query("DELETE FROM memory WHERE workspace_id=$1")
        .bind(&workspace)
        .execute(&pool)
        .await
        .unwrap();
}
