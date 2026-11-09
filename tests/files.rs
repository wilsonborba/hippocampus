use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use hippocampus::{
    dal::{
        local::{store::Store, test_database},
        remote::{documents::Documents, files::FsmClient},
    },
    domain::services::memories::MemoryService,
    presentation::api::{AppState, router},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};
type StoredBytes = HashMap<String, (Vec<u8>, String)>;
#[derive(Clone, Default)]
struct FsmDouble {
    files: Arc<Mutex<StoredBytes>>,
    fail_upload: Arc<AtomicBool>,
    fail_download: Arc<AtomicBool>,
    uploads: Arc<AtomicUsize>,
    downloads: Arc<AtomicUsize>,
}
async fn upload(State(s): State<FsmDouble>, headers: HeaderMap, mut form: Multipart) -> Response {
    assert_eq!(headers["authorization"], "Bearer fixture-only");
    s.uploads.fetch_add(1, Ordering::SeqCst);
    if s.fail_upload.load(Ordering::SeqCst) {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let mut bytes = vec![];
    let mut mime = "application/octet-stream".to_owned();
    while let Some(field) = form.next_field().await.unwrap() {
        match field.name().unwrap_or("") {
            "album" => assert!(field.text().await.unwrap().starts_with("memory-")),
            "force" => assert_eq!(field.text().await.unwrap(), "false"),
            "file" => {
                mime = field.content_type().unwrap().into();
                bytes = field.bytes().await.unwrap().to_vec();
            }
            _ => {}
        }
    }
    let key = format!("fixture-{}", uuid::Uuid::new_v4().simple());
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    s.files
        .lock()
        .unwrap()
        .insert(key.clone(), (bytes.clone(), mime.clone()));
    (StatusCode::CREATED,Json(json!({"item":{"key":key,"content_type":mime,"size_bytes":bytes.len(),"checksum_sha256":checksum}}))).into_response()
}
async fn download(
    State(s): State<FsmDouble>,
    Path(key): Path<String>,
    headers: HeaderMap,
) -> Response {
    assert_eq!(headers["authorization"], "Bearer fixture-only");
    s.downloads.fetch_add(1, Ordering::SeqCst);
    if s.fail_download.load(Ordering::SeqCst) {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Some((bytes, mime)) = s.files.lock().unwrap().get(&key) {
        return ([("content-type", mime.clone())], bytes.clone()).into_response();
    }
    StatusCode::NOT_FOUND.into_response()
}
async fn serve(app: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), task)
}
fn form(
    workspace: &str,
    name: &str,
    text: &str,
    user: Option<&str>,
    app: bool,
) -> reqwest::multipart::Form {
    let mut form = reqwest::multipart::Form::new()
        .text("workspace_id", workspace.to_owned())
        .text("extracted_text", text.to_owned())
        .part(
            "file",
            reqwest::multipart::Part::bytes(b"raw-file-bytes".to_vec())
                .file_name(name.to_owned())
                .mime_str("text/plain")
                .unwrap(),
        );
    if app {
        form = form.text("app_id", "cortex");
    }
    if let Some(user) = user {
        form = form.text("user_id", user.to_owned());
    }
    form
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test; FSM is a local double"]
async fn upload_empty_text_download_scope_and_failure() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let fsm = FsmDouble::default();
    let (fsm_url, fsm_task) = serve(
        Router::new()
            .route("/hippocampus/media", post(upload))
            .route("/hippocampus/media/{key}", get(download))
            .with_state(fsm.clone()),
    )
    .await;
    let documents = Documents::default();
    let state = AppState {
        service: MemoryService {
            store: Store { pool: pool.clone() },
        },
        documents: documents.clone(),
        api_keys: Default::default(),
        filestore: Arc::new(FsmClient::new(
            fsm_url,
            "fixture-only".into(),
            "hippocampus".into(),
        )),
    };
    let (api, api_task) = serve(router(state)).await;
    let c = reqwest::Client::new();
    let workspace = format!("files-{}", uuid::Uuid::new_v4().simple());
    let mut ids = vec![];
    for (name, text, user) in [
        ("sample.txt", "The extracted passage.", Some("user-a")),
        ("empty.txt", "", None),
    ] {
        let response = c
            .post(format!("{api}/api/v1/files"))
            .multipart(form(&workspace, name, text, user, true))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 201);
        let body: Value = response.json().await.unwrap();
        let mid = body["data"]["memory_id"].as_str().unwrap().to_owned();
        let rid = body["data"]["resource_id"].as_str().unwrap().to_owned();
        let memory: Value = c
            .get(format!("{api}/api/v1/memories/{mid}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(memory["data"]["memory_type"], "file");
        assert_eq!(memory["data"]["title"], name);
        assert_eq!(memory["data"]["content"], text);
        assert_eq!(memory["data"]["app_id"], "cortex");
        assert_eq!(memory["data"]["user_id"], json!(user));
        assert!(documents.get(&mid).await.is_none());
        let resource: Value = c
            .get(format!("{api}/api/v1/resources/{rid}"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(resource["data"]["size_bytes"], 14);
        assert_eq!(
            resource["data"]["checksum"],
            format!("{:x}", Sha256::digest(b"raw-file-bytes"))
        );
        let metadata_denied = c
            .get(format!("{api}/api/v1/resources/{rid}"))
            .query(&[
                ("workspace_id", workspace.as_str()),
                ("app_id", "cortex"),
                ("user_id", "other-user"),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(metadata_denied.status().as_u16(), 404);
        let before = fsm.downloads.load(Ordering::SeqCst);
        let denied = c
            .get(format!("{api}/api/v1/resources/{rid}/content"))
            .query(&[
                ("workspace_id", workspace.as_str()),
                ("app_id", "cortex"),
                ("user_id", "other-user"),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(denied.status().as_u16(), 404);
        assert_eq!(fsm.downloads.load(Ordering::SeqCst), before);
        // Resource URIs cannot redirect the adapter to another service.
        sqlx::query("UPDATE resource SET uri='http://127.0.0.1:9/not-used' WHERE id=$1")
            .bind(&rid)
            .execute(&pool)
            .await
            .unwrap();
        let downloaded = c
            .get(format!("{api}/api/v1/resources/{rid}/content"))
            .query(&[
                ("workspace_id", workspace.as_str()),
                ("app_id", "cortex"),
                ("user_id", user.unwrap_or("")),
            ])
            .send()
            .await
            .unwrap();
        assert_eq!(downloaded.status().as_u16(), 200);
        assert_eq!(downloaded.headers()["content-type"], "text/plain");
        assert!(
            downloaded.headers()["content-disposition"]
                .to_str()
                .unwrap()
                .contains("attachment")
        );
        assert_eq!(
            downloaded.bytes().await.unwrap().as_ref(),
            b"raw-file-bytes"
        );
        ids.push((mid, rid));
    }
    let before = fsm.uploads.load(Ordering::SeqCst);
    let bad = c
        .post(format!("{api}/api/v1/files"))
        .multipart(form(&workspace, "bad.txt", "", None, false))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status().as_u16(), 400);
    assert_eq!(fsm.uploads.load(Ordering::SeqCst), before);
    fsm.fail_upload.store(true, Ordering::SeqCst);
    let failed = c
        .post(format!("{api}/api/v1/files"))
        .multipart(form(&workspace, "failed.txt", "", None, true))
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status().as_u16(), 502);
    assert_eq!(
        failed.json::<Value>().await.unwrap()["error"]["code"],
        "filestore_error"
    );
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM memory WHERE workspace_id=$1")
        .bind(&workspace)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 2);
    fsm.fail_download.store(true, Ordering::SeqCst);
    let failed = c
        .get(format!("{api}/api/v1/resources/{}/content", ids[0].1))
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status().as_u16(), 502);
    for (mid, rid) in ids {
        sqlx::query("DELETE FROM memory_resource WHERE memory_id=$1")
            .bind(&mid)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM memory_event WHERE memory_id=$1")
            .bind(&mid)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM memory WHERE id=$1")
            .bind(&mid)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM resource WHERE id=$1")
            .bind(&rid)
            .execute(&pool)
            .await
            .unwrap();
    }
    api_task.abort();
    fsm_task.abort();
}

#[tokio::test]
async fn missing_fsm_configuration_is_an_explicit_error() {
    use hippocampus::domain::models::files::FileStore;
    let result = FsmClient::default()
        .upload("fixture", "file.txt", "text/plain", b"fixture")
        .await;
    assert!(
        matches!(result,Err(error) if error.status==StatusCode::SERVICE_UNAVAILABLE && error.name=="filestore_not_configured")
    );
}
