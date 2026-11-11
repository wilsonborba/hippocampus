use hippocampus::{
    dal::{
        local::{
            store::{Store, Table},
            test_database,
        },
        remote::documents::Documents,
    },
    domain::{
        models::{persona_schema::SchemaDraft, scope::WriteScope},
        services::{
            persona_schemas::draft,
            personas::{create, delete, list, matches, update},
        },
    },
};
use serde_json::{Value, json};
#[tokio::test]
#[ignore = "requires hippocampus_v2_test PostgreSQL and CouchDB test databases"]
async fn published_versions_guardrail_old_reads_and_real_document_delete() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let docs = Documents::from_environment().await;
    let w = format!("schema-{}", uuid::Uuid::new_v4().simple());
    let fields = json!([{"name":"email","type":"string","required":true,"in_context":false},{"name":"preference","type":"string","required":false,"in_context":true}]);
    let schema: SchemaDraft =
        serde_json::from_value(json!({"schema_name":"person","version":1,"fields":fields}))
            .unwrap();
    assert_eq!(draft(&store, schema).await.unwrap().status, "draft");
    let request = |name: &str, attributes: Value| {
        serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":"user","entity_type":"person","canonical_name":name,"aliases":["Old Alias"],"attributes":attributes})).unwrap()
    };
    assert!(
        create(
            &store,
            &docs,
            request("Unpublished", json!({"email":"fixture@example.invalid"}))
        )
        .await
        .is_err()
    );
    assert_eq!(
        store.publish_schema("person", 1).await.unwrap().status,
        "published"
    );
    for (attributes, path) in [
        (json!({}), "email"),
        (json!({"email":3}), "email"),
        (
            json!({"email":"fixture@example.invalid","unknown":true}),
            "unknown",
        ),
        (Value::Null, "$"),
    ] {
        let error = create(&store, &docs, request("Invalid", attributes))
            .await
            .err()
            .unwrap();
        assert_eq!(error.status, axum::http::StatusCode::UNPROCESSABLE_ENTITY);
        assert!(
            error.details["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["field_path"] == path)
        );
    }
    let p = create(
        &store,
        &docs,
        request(
            "Old Persona",
            json!({"email":"fixture@example.invalid","preference":"short"}),
        ),
    )
    .await
    .unwrap();
    let old = p["id"].as_str().unwrap().to_owned();
    assert_eq!(p["schema_version"], 1);
    assert_eq!(docs.get_persona(&old).await.unwrap().unwrap()["_id"], old);
    let schema:SchemaDraft=serde_json::from_value(json!({"schema_name":"person","version":2,"fields":[{"name":"email","type":"string","required":true,"in_context":false},{"name":"rule","type":"boolean","required":true,"in_context":true}]})).unwrap();
    draft(&store, schema).await.unwrap();
    store.publish_schema("person", 2).await.unwrap();
    assert_eq!(store.schema("person", None).await.unwrap().version, 2);
    let replacement =
        serde_json::from_value(json!({"schema_name":"person","version":1,"fields":fields}))
            .unwrap();
    assert_eq!(
        draft(&store, replacement).await.err().unwrap().status,
        axum::http::StatusCode::CONFLICT
    );
    assert!(
        create(
            &store,
            &docs,
            request(
                "Missing New Field",
                json!({"email":"fixture@example.invalid"})
            )
        )
        .await
        .is_err()
    );
    let new = create(
        &store,
        &docs,
        request(
            "New Persona",
            json!({"email":"new@example.invalid","rule":true}),
        ),
    )
    .await
    .unwrap();
    let new_id = new["id"].as_str().unwrap().to_owned();
    assert_eq!(new["schema_version"], 2);
    let scope =
        serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":"user"}))
            .unwrap();
    let rows = list(&store, &docs, scope).await.unwrap();
    assert_eq!(rows.len(), 2);
    let old_row = rows.iter().find(|p| p["id"] == old).unwrap();
    assert_eq!(old_row["schema_version"], 1);
    assert_eq!(old_row["attributes"]["preference"], "short");
    let change=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":"user","attributes":{"email":"changed@example.invalid","preference":"clear"}})).unwrap();
    assert_eq!(
        update(&store, &docs, &old, change).await.unwrap()["schema_version"],
        1
    );
    let query = serde_json::from_value(
        json!({"workspace_id":w,"app_id":"fixture","user_id":"user","text":"Old Persona"}),
    )
    .unwrap();
    let found = matches(&store, &docs, query).await.unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0]["attributes"]["preference"], "clear");
    {
        use axum::{
            Router,
            body::{Body, to_bytes},
            http::{Request, StatusCode},
            routing::any,
        };
        use hippocampus::{
            dal::remote::{embeddings::DisabledEmbedding, files::FsmClient},
            domain::services::memories::MemoryService,
            presentation::api::{AppState, router},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().fallback(any(|| async { StatusCode::SERVICE_UNAVAILABLE })),
            )
            .await
            .unwrap()
        });
        let failing = Documents::couchdb(
            format!("http://{address}"),
            "fixture".into(),
            "fixture-only".into(),
        );
        assert!(
            create(
                &store,
                &failing,
                request(
                    "Store Failure",
                    json!({"email":"fixture@example.invalid","rule":true})
                )
            )
            .await
            .is_err()
        );
        let scope = WriteScope {
            workspace_id: w.clone(),
            app_id: "fixture".into(),
            user_id: Some("user".into()),
        };
        assert!(delete(&store, &failing, &old, scope).await.is_err());
        assert!(store.get(Table::Entity, &old).await.unwrap().is_some());
        assert!(docs.get_persona(&old).await.unwrap().is_some());
        task.abort();
        let app = router(AppState {
            service: MemoryService {
                store: store.clone(),
            },
            documents: docs.clone(),
            filestore: Arc::new(FsmClient::default()),
            embedding: Arc::new(DisabledEmbedding),
            api_keys: Default::default(),
        });
        for path in [
            "/api/v1/persona-schemas/person",
            "/api/v1/persona-schemas/person/versions/1",
        ] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let body: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap())
                    .unwrap();
            assert!(!body["data"]["fields"].as_array().unwrap().is_empty());
        }
        let body = json!({"workspace_id":w,"app_id":"fixture","user_id":"user","entity_type":"person","canonical_name":"HTTP Invalid","attributes":{"email":4}});
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/entities")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert!(
            body["error"]["details"]["errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["field_path"] == "email")
        );
        let body = json!({"schema_name":"bot","version":1,"fields":[{"name":"voice","type":"string","required":false,"in_context":true}]});
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/v1/persona-schemas")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let response = app
            .oneshot(
                Request::post("/api/v1/persona-schemas/bot/versions/1/publish")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let memory_service = hippocampus::domain::services::memories::MemoryService {
        store: store.clone(),
    };
    let memory=memory_service.create(serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":"user","title":"Retained memory","entities":[{"entity_type":"person","canonical_name":"Old Persona"}]})).unwrap()).await.unwrap();
    let memory_id = memory["id"].as_str().unwrap().to_owned();
    let linked: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM memory_entity WHERE memory_id=$1 AND entity_id=$2",
    )
    .bind(&memory_id)
    .bind(&old)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(linked, 1);
    let wrong = WriteScope {
        workspace_id: w.clone(),
        app_id: "fixture".into(),
        user_id: Some("other".into()),
    };
    assert!(delete(&store, &docs, &old, wrong).await.is_err());
    assert!(docs.get_persona(&old).await.unwrap().is_some());
    let scope = || WriteScope {
        workspace_id: w.clone(),
        app_id: "fixture".into(),
        user_id: Some("user".into()),
    };
    {
        use axum::{
            body::Body,
            http::{Request, StatusCode},
        };
        use hippocampus::{
            dal::remote::{embeddings::DisabledEmbedding, files::FsmClient},
            presentation::api::{AppState, router},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let app = router(AppState {
            service: memory_service.clone(),
            documents: docs.clone(),
            filestore: Arc::new(FsmClient::default()),
            embedding: Arc::new(DisabledEmbedding),
            api_keys: Default::default(),
        });
        let response = app
            .oneshot(
                Request::delete(format!(
                    "/api/v1/entities/{old}?workspace_id={w}&app_id=fixture&user_id=user"
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
    let aliases: i64 = sqlx::query_scalar("SELECT count(*) FROM entity_alias WHERE entity_id=$1")
        .bind(&old)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(aliases, 0);
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM memory_entity WHERE entity_id=$1")
        .bind(&old)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(links, 0);
    assert!(
        store
            .get(Table::Memory, &memory_id)
            .await
            .unwrap()
            .is_some()
    );
    let word_matches:i64=sqlx::query_scalar("SELECT count(*) FROM memory WHERE id=$1 AND search_document @@ plainto_tsquery('simple','Old Persona')").bind(&memory_id).fetch_one(&pool).await.unwrap();
    assert_eq!(word_matches, 0);
    hippocampus::domain::services::deletion::delete(
        &store,
        &hippocampus::dal::remote::files::FsmClient::default(),
        &docs,
        hippocampus::domain::models::deletion::DeleteRequest {
            scope: scope(),
            memory_ids: vec![memory_id],
        },
    )
    .await
    .unwrap();
    assert!(store.get(Table::Entity, &old).await.unwrap().is_none());
    assert!(docs.get_persona(&old).await.unwrap().is_none());
    delete(&store, &docs, &old, scope()).await.unwrap();
    delete(&store, &docs, &new_id, scope()).await.unwrap();
    let invalid: i64 = sqlx::query_scalar("SELECT count(*) FROM entity WHERE workspace_id=$1")
        .bind(&w)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(invalid, 0);
    sqlx::query("DELETE FROM persona_schema WHERE (schema_name='person' AND version IN (1,2)) OR (schema_name='bot' AND version=1)")
        .execute(&pool)
        .await
        .unwrap();
}
