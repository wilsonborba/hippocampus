use hippocampus::{
    dal::{
        local::{store::Store, test_database},
        remote::embeddings::DisabledEmbedding,
    },
    domain::{
        models::{
            api::MemoryCreateRequest,
            embedding::{EmbeddingSource, EmbeddingStore, Pending},
            search::SearchRequest,
        },
        services::{memories::MemoryService, search::search},
    },
};
use serde_json::{Value, json};
struct Model;
impl EmbeddingSource for Model {
    fn name(&self) -> &str {
        "search-fixture"
    }
    fn embed<'a>(&'a self, _: &'a str) -> Pending<'a, Option<Vec<f64>>> {
        Box::pin(async { Some(vec![1.0, 0.0]) })
    }
}
fn req(w: &str, extra: Value) -> SearchRequest {
    let mut v = json!({"workspace_id":w,"app_id":"fixture","user_id":"alice"});
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(v).unwrap()
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test"]
async fn filtered_search_word_vector_history_and_expansion() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let w = format!("search-{}", uuid::Uuid::new_v4().simple());
    let mut ids = vec![];
    for (title, user, tags) in [
        (
            "needle",
            Some("alice"),
            vec![format!("{w}:a"), format!("{w}:b")],
        ),
        ("linked", Some("alice"), vec![format!("{w}:a")]),
        (
            "private",
            Some("bob"),
            vec![format!("{w}:a"), format!("{w}:b")],
        ),
        ("oldneedle", Some("alice"), vec![]),
        ("machine", None, vec![]),
    ] {
        let r: MemoryCreateRequest = serde_json::from_value(
            json!({"workspace_id":w,"app_id":"fixture","user_id":user,"title":title,"tags":tags}),
        )
        .unwrap();
        ids.push(
            service.create(r).await.unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    for _ in 0..25 {
        let r: MemoryCreateRequest = serde_json::from_value(
            json!({"workspace_id":w,"app_id":"fixture","user_id":"alice","title":"distractor"}),
        )
        .unwrap();
        ids.push(
            service.create(r).await.unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let run = |r| search(&store, &DisabledEmbedding, r);
    let a = run(req(&w, json!({"text":"needle"}))).await.unwrap();
    assert_eq!(a.as_array().unwrap().len(), 1);
    assert_eq!(a[0]["memory"]["id"], ids[0]);
    assert_eq!(
        run(req(&w, json!({"tags":[format!("{w}:a"),format!("{w}:b")]})))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        run(req(&w, json!({"text":"no_match"}))).await.unwrap(),
        json!([])
    );
    sqlx::query("UPDATE memory SET status='superseded' WHERE id=$1")
        .bind(&ids[3])
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        run(req(&w, json!({"text":"oldneedle"}))).await.unwrap(),
        json!([])
    );
    assert_eq!(
        run(req(&w, json!({"text":"oldneedle","historical":true})))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Name changes and link changes refresh the full-text document without changing content timestamps.
    let before: String = sqlx::query_scalar("SELECT updated_at::text FROM memory WHERE id=$1")
        .bind(&ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE tag SET canonical_name=$3 WHERE canonical_name=$2 AND id IN (SELECT tag_id FROM memory_tag WHERE memory_id=$1)").bind(&ids[0]).bind(format!("{w}:b")).bind(format!("{w}:renamedneedle")).execute(&pool).await.unwrap();
    assert_eq!(
        run(req(&w, json!({"text":"renamedneedle"})))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let entity = service
        .associate_entity(
            &ids[0],
            serde_json::from_value(
                json!({"entity_type":"concept","canonical_name":format!("entity-{w}")}),
            )
            .unwrap(),
            None,
        )
        .await
        .unwrap();
    let entity_id = entity["id"].as_str().unwrap();
    sqlx::query("UPDATE entity SET canonical_name='entityrenamedneedle' WHERE id=$1")
        .bind(entity_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(run(req(&w,json!({"text":"entityrenamedneedle","entity_ids":[entity_id],"memory_types":["semantic"]}))).await.unwrap().as_array().unwrap().len(),1);
    assert_eq!(
        run(req(
            &w,
            json!({"text":"entityrenamedneedle","entity_ids":["missing"]})
        ))
        .await
        .unwrap(),
        json!([])
    );
    let after: String = sqlx::query_scalar("SELECT updated_at::text FROM memory WHERE id=$1")
        .bind(&ids[0])
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
    service
        .relationship(
            &ids[0],
            serde_json::from_value(json!({"target_memory_id":ids[1],"relation_type":"related_to"}))
                .unwrap(),
            true,
        )
        .await
        .unwrap();
    service
        .relationship(
            &ids[0],
            serde_json::from_value(json!({"target_memory_id":ids[2],"relation_type":"related_to"}))
                .unwrap(),
            true,
        )
        .await
        .unwrap();
    let expanded = run(req(&w, json!({"text":"needle","expand":3})))
        .await
        .unwrap();
    assert_eq!(expanded.as_array().unwrap().len(), 2);
    assert_eq!(expanded[1]["match_reasons"], json!(["relationship"]));
    for (i, v) in [
        (0, vec![0.0, 1.0]),
        (1, vec![1.0, 0.0]),
        (2, vec![1.0, 0.0]),
    ] {
        store
            .save(&ids[i], "search-fixture", &v, "fixture", chrono::Utc::now())
            .await
            .unwrap();
    }
    let semantic = search(&store, &Model, req(&w, json!({"text":"needle"})))
        .await
        .unwrap();
    assert_eq!(semantic[0]["memory"]["id"], ids[0]);
    assert_eq!(semantic[0]["match_reasons"], json!(["word", "semantic"]));
    assert_eq!(semantic.as_array().unwrap().len(), 2);
    let only = search(&store, &Model, req(&w, json!({"text":"unseen"})))
        .await
        .unwrap();
    assert_eq!(only[0]["memory"]["id"], ids[1]);
    assert_eq!(
        run(req(&w, json!({"user_id":null})))
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(run(req(&w, json!({"expand":4}))).await.is_err());
    {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use hippocampus::{
            dal::remote::{documents::Documents, files::FsmClient},
            presentation::api::{AppState, router},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let app = router(AppState {
            embedding: Arc::new(DisabledEmbedding),
            filestore: Arc::new(FsmClient::default()),
            service: service.clone(),
            documents: Documents::default(),
            api_keys: Default::default(),
        });
        let response=app.oneshot(Request::post("/api/v1/search").header("content-type","application/json").body(Body::from(json!({"workspace_id":w,"app_id":"fixture","user_id":"alice","text":"needle"}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert_eq!(body["data"][0]["memory"]["id"], ids[0]);
        for command in ["search", "recall"] {
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_hippocampus"))
                .args(["--json", command, "needle", "--workspace", &w])
                .output()
                .unwrap();
            assert!(output.status.success());
            let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(rows[0]["id"], ids[0]);
        }
    }
    sqlx::query("DELETE FROM memory_relationship WHERE source_memory_id=ANY($1) OR target_memory_id=ANY($1)").bind(&ids).execute(&pool).await.unwrap();
    for table in [
        "memory_embedding",
        "memory_tag",
        "memory_entity",
        "memory_event",
    ] {
        sqlx::query(&format!("DELETE FROM {table} WHERE memory_id=ANY($1)"))
            .bind(&ids)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM memory WHERE id=ANY($1)")
        .bind(&ids)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn http_requires_scope_and_valid_expand() {
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use hippocampus::{
        dal::remote::{documents::Documents, files::FsmClient},
        presentation::api::{AppState, router},
    };
    use std::sync::Arc;
    use tower::ServiceExt;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/hippocampus_v2_test")
        .unwrap();
    let app = router(AppState {
        embedding: Arc::new(DisabledEmbedding),
        filestore: Arc::new(FsmClient::default()),
        service: MemoryService {
            store: Store { pool },
        },
        documents: Documents::default(),
        api_keys: Default::default(),
    });
    for body in [
        json!({}),
        json!({"workspace_id":"w","expand":4}),
        json!({"workspace_id":"w","expand":-1}),
        json!({"workspace_id":"w","limit":0}),
        json!({"workspace_id":"","tags":[4]}),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/search")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let expected = if body["expand"] == 4 || body["limit"] == 0 {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        assert_eq!(response.status(), expected);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
        if expected == StatusCode::UNPROCESSABLE_ENTITY {
            assert!(value["detail"].is_array());
        }
    }
    for route in ["/api/v1/recall", "/api/v1/search/semantic"] {
        let response = app
            .clone()
            .oneshot(Request::post(route).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
