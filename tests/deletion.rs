use axum::{
    Router,
    body::{Body, to_bytes},
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::delete,
};
use hippocampus::{
    dal::{
        local::{
            store::{Store, Table},
            test_database,
        },
        remote::{documents::Documents, embeddings::DisabledEmbedding, files::FsmClient},
    },
    domain::{
        models::{
            api::MemoryCreateRequest, embedding::EmbeddingStore, files::StoredFile,
            scope::WriteScope,
        },
        services::memories::MemoryService,
    },
    presentation::api::{AppState, router},
};
use serde_json::{Value, json};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use tower::ServiceExt;
#[derive(Clone, Default)]
struct Fsm {
    calls: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
    missing: Arc<AtomicBool>,
}
async fn remove(State(s): State<Fsm>, headers: HeaderMap) -> StatusCode {
    assert_eq!(headers["authorization"], "Bearer fixture-only");
    s.calls.fetch_add(1, Ordering::SeqCst);
    if s.fail.load(Ordering::SeqCst) {
        StatusCode::SERVICE_UNAVAILABLE
    } else if s.missing.load(Ordering::SeqCst) {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::NO_CONTENT
    }
}
async fn call(app: &Router, w: &str, ids: Vec<String>, user: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::post("/api/v1/memories/delete")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"workspace_id":w,"app_id":"fixture","user_id":user,"memory_ids":ids})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let data =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    (status, data)
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test and local FSM double"]
async fn shared_file_last_link_failure_retry_and_scope() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let w = format!("delete-{}", uuid::Uuid::new_v4().simple());
    let scope = WriteScope {
        workspace_id: w.clone(),
        app_id: "fixture".into(),
        user_id: Some("alice".into()),
    };
    let first = store
        .save_file_record(
            &format!("{w}-file"),
            &scope,
            "fixture.txt",
            "needle",
            &StoredFile {
                key: "fixture-file".into(),
                uri: "http://invalid.invalid/no-request".into(),
                content_type: "text/plain".into(),
                size_bytes: 1,
                checksum: "fixture".into(),
            },
        )
        .await
        .unwrap();
    let mut ids = vec![first.memory_id.clone()];
    for user in ["alice", "bob"] {
        let r:MemoryCreateRequest=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":user,"title":"shared","tags":[format!("shared:{w}")]})).unwrap();
        ids.push(
            service.create(r).await.unwrap()["id"]
                .as_str()
                .unwrap()
                .into(),
        );
    }
    let file = store
        .get(Table::Resource, &first.resource_id)
        .await
        .unwrap()
        .unwrap();
    service
        .link_resource(&ids[1], &file, "references", None)
        .await
        .unwrap();
    store
        .save(
            &ids[0],
            "fixture",
            &[1.0, 0.0],
            "fixture",
            chrono::Utc::now(),
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
    let documents = Documents::default();
    documents.put(&ids[0], json!({"content":"fixture"})).await;
    sqlx::query("UPDATE memory SET document_store='couchdb',document_id=$1 WHERE id=$1")
        .bind(&ids[0])
        .execute(&pool)
        .await
        .unwrap();
    let fsm = Fsm::default();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = fsm.clone();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new()
                .route("/hippocampus/media/{key}", delete(remove))
                .with_state(server),
        )
        .await
        .unwrap()
    });
    let app = router(AppState {
        service: service.clone(),
        documents: documents.clone(),
        embedding: Arc::new(DisabledEmbedding),
        api_keys: Default::default(),
        filestore: Arc::new(FsmClient::new(
            format!("http://{address}"),
            "fixture-only".into(),
            "hippocampus".into(),
        )),
    });
    // Mixed valid and wrong-scope requests must roll back every id before any remote call.
    assert_eq!(
        call(&app, &w, vec![ids[0].clone(), ids[2].clone()], "alice")
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert!(store.get(Table::Memory, &ids[0]).await.unwrap().is_some());
    assert_eq!(fsm.calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        call(&app, &w, vec![ids[0].clone()], "alice").await.0,
        StatusCode::OK
    );
    assert!(store.get(Table::Memory, &ids[0]).await.unwrap().is_none());
    assert!(
        store
            .get(Table::Resource, &first.resource_id)
            .await
            .unwrap()
            .is_some()
    );
    assert!(documents.get(&ids[0]).await.is_none());
    assert_eq!(fsm.calls.load(Ordering::SeqCst), 0);
    assert!(store.relationships(&ids[2]).await.unwrap().is_empty());
    fsm.fail.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&app, &w, vec![ids[1].clone()], "alice").await.0,
        StatusCode::BAD_GATEWAY
    );
    assert!(store.get(Table::Memory, &ids[1]).await.unwrap().is_some());
    assert!(
        store
            .get(Table::Resource, &first.resource_id)
            .await
            .unwrap()
            .is_some()
    );
    fsm.fail.store(false, Ordering::SeqCst);
    let (status, result) = call(
        &app,
        &w,
        vec![ids[1].clone(), ids[1].clone(), "absent".into()],
        "alice",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(result["data"], json!([ids[1]]));
    assert!(
        store
            .get(Table::Resource, &first.resource_id)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(fsm.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        call(&app, &w, vec![ids[1].clone()], "alice").await.1["data"],
        json!([])
    );
    assert_eq!(fsm.calls.load(Ordering::SeqCst), 2);
    // A remote 404 is success after a partial external delete or a client retry.
    let other = store
        .save_file_record(
            &format!("{w}-missing"),
            &scope,
            "fixture.txt",
            "",
            &StoredFile {
                key: "already-gone".into(),
                uri: "fixture://missing".into(),
                content_type: "text/plain".into(),
                size_bytes: 0,
                checksum: "fixture".into(),
            },
        )
        .await
        .unwrap();
    fsm.missing.store(true, Ordering::SeqCst);
    assert_eq!(
        call(&app, &w, vec![other.memory_id], "alice").await.0,
        StatusCode::OK
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_hippocampus"))
        .args([
            "--json",
            "memory",
            "delete",
            &ids[2],
            "--hard",
            "--yes",
            "--workspace",
            &w,
            "--app",
            "fixture",
            "--user",
            "bob",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result, json!({"memory_id":ids[2],"status":"deleted"}));
    assert_eq!(
        call(&app, &w, vec![ids[2].clone()], "bob").await.1["data"],
        json!([])
    );
    task.abort();
}
