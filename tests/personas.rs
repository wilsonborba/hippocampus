use hippocampus::{
    dal::local::{store::Store, test_database},
    domain::{
        models::personas::*,
        services::personas::{create, list, matches, update},
    },
};
use serde_json::{Value, json};
fn req(w: &str, user: Option<&str>, name: &str, is_self: bool) -> PersonaCreate {
    serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":user,"entity_type":"person","canonical_name":name,"aliases":["Ally"],"is_self":is_self})).unwrap()
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test"]
async fn scope_self_alias_and_boundaries() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let docs = hippocampus::dal::remote::documents::Documents::default();
    let w = format!("persona-{}", uuid::Uuid::new_v4().simple());
    let mut ids = vec![];
    for user in [Some("alice"), Some("bob"), None] {
        let p = create(&store, &docs, req(&w, user, "Alice Smith", true))
            .await
            .unwrap();
        ids.push(p["id"].as_str().unwrap().to_owned());
        let conflict = create(&store, &docs, req(&w, user, "Different", true))
            .await
            .err()
            .unwrap();
        assert_eq!(conflict.status, axum::http::StatusCode::CONFLICT);
    }
    assert!(
        create(
            &store,
            &docs,
            req(&format!("{w}-other"), Some("alice"), "Different", true)
        )
        .await
        .is_err()
    );
    let other_app=serde_json::from_value(json!({"workspace_id":w,"app_id":"other","user_id":"alice","entity_type":"bot","canonical_name":"Assistant","is_self":true})).unwrap();
    ids.push(
        create(&store, &docs, other_app).await.unwrap()["id"]
            .as_str()
            .unwrap()
            .into(),
    );
    for (i, user) in [Some("alice"), Some("bob"), None].into_iter().enumerate() {
        let r=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":user,"text":"Ask ALLY, and Alice Smith."})).unwrap();
        let found = matches(&store, &docs, r).await.unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["id"], ids[i]);
        let r=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":user,"text":"Alliance and Alice Smithsonian"})).unwrap();
        assert!(matches(&store, &docs, r).await.unwrap().is_empty());
    }
    let all = serde_json::from_value(json!({"workspace_id":w})).unwrap();
    assert_eq!(list(&store, &docs, all).await.unwrap().len(), 4);
    let wrong: PersonaUpdate = serde_json::from_value(
        json!({"workspace_id":w,"app_id":"fixture","user_id":"bob","canonical_name":"Changed"}),
    )
    .unwrap();
    assert!(update(&store, &docs, &ids[0], wrong).await.is_err());
    let change=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":"alice","canonical_name":"New Name","aliases":["Newest"],"is_self":false})).unwrap();
    let changed = update(&store, &docs, &ids[0], change).await.unwrap();
    assert_eq!(changed["canonical_name"], "New Name");
    assert_eq!(changed["aliases"], json!(["Newest"]));
    assert_eq!(changed["is_self"], false);
    let r = serde_json::from_value(
        json!({"workspace_id":w,"app_id":"fixture","user_id":"alice","text":"Newest!"}),
    )
    .unwrap();
    assert_eq!(matches(&store, &docs, r).await.unwrap()[0]["id"], ids[0]);
    {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use hippocampus::{
            dal::remote::{documents::Documents, embeddings::DisabledEmbedding, files::FsmClient},
            domain::services::memories::MemoryService,
            presentation::api::{AppState, router},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let app = router(AppState {
            embedding: Arc::new(DisabledEmbedding),
            filestore: Arc::new(FsmClient::default()),
            documents: Documents::default(),
            service: MemoryService {
                store: store.clone(),
            },
            api_keys: Default::default(),
        });
        for (path, status) in [
            (
                format!("entities?workspace_id={w}&app_id=fixture&user_id=alice"),
                StatusCode::OK,
            ),
            (
                format!(
                    "entities/{}?workspace_id={w}&app_id=fixture&user_id=alice",
                    ids[0]
                ),
                StatusCode::OK,
            ),
            (
                format!(
                    "entities/{}?workspace_id={w}&app_id=fixture&user_id=bob",
                    ids[0]
                ),
                StatusCode::NOT_FOUND,
            ),
            (format!("entities/{}", ids[0]), StatusCode::BAD_REQUEST),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::get(format!("/api/v1/{path}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), status);
        }
        let body = json!({"workspace_id":w,"app_id":"fixture","user_id":"alice","text":"Newest"});
        let response = app
            .oneshot(
                Request::post("/api/v1/entities/match")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(body["data"][0]["id"], ids[0]);
    }
    sqlx::query("DELETE FROM entity_alias WHERE entity_id=ANY($1)")
        .bind(&ids)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM entity WHERE id=ANY($1)")
        .bind(&ids)
        .execute(&pool)
        .await
        .unwrap();
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test; migration runs on temporary tables only"]
async fn migration_splits_shared_entities_and_preserves_aliases() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql("CREATE TEMP TABLE entity (LIKE public.entity INCLUDING DEFAULTS); ALTER TABLE entity DROP COLUMN workspace_id,DROP COLUMN app_id,DROP COLUMN user_id,DROP COLUMN is_self; ALTER TABLE entity ADD CONSTRAINT uq_entity_type_normalized_name UNIQUE(entity_type,normalized_name); CREATE TEMP TABLE memory (id text,workspace_id text,app_id text,user_id text); CREATE TEMP TABLE memory_entity (id text,memory_id text,entity_id text); CREATE TEMP TABLE entity_alias(LIKE public.entity_alias); INSERT INTO entity(id,entity_type,canonical_name,normalized_name,status,metadata_json,created_at,updated_at) VALUES ('shared','person','Alice','alice','active','{}',now(),now()),('unlinked','bot','Bot','bot','active','{}',now(),now()); INSERT INTO memory VALUES ('a','work','app','alice'),('b','work','app','bob'); INSERT INTO memory_entity VALUES ('a','a','shared'),('b','b','shared'); INSERT INTO entity_alias(id,entity_id,alias,normalized_alias,created_at) VALUES ('alias','shared','Ally','ally',now());").execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!("../migrations/0004_scoped_entities.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    let rows: Vec<Value> = sqlx::query_scalar("SELECT row_to_json(e) FROM entity e ORDER BY id")
        .fetch_all(&mut *tx)
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM memory_entity me JOIN entity e ON e.id=me.entity_id JOIN memory m ON m.id=me.memory_id WHERE e.user_id=m.user_id AND e.app_id=m.app_id AND e.workspace_id=m.workspace_id").fetch_one(&mut *tx).await.unwrap();
    assert_eq!(count, 2);
    let aliases: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_alias")
        .fetch_one(&mut *tx)
        .await
        .unwrap();
    assert_eq!(aliases, 2);
    tx.rollback().await.unwrap();
}
