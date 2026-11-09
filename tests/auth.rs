use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use hippocampus::{
    dal::{local::store::Store, remote::documents::Documents},
    domain::services::memories::MemoryService,
    presentation::api::{AppState, router},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};
use tower::ServiceExt;
#[tokio::test]
async fn optional_bearer_contract() {
    // A lazy pool proves rejected calls never use PostgreSQL.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/hippocampus_v2_test")
        .unwrap();
    let state = AppState {
        embedding: hippocampus::dal::remote::embeddings::configured(&Default::default()),
        filestore: std::sync::Arc::new(hippocampus::dal::remote::files::FsmClient::default()),
        service: MemoryService {
            store: Store { pool },
        },
        documents: Documents::default(),
        api_keys: Arc::new(BTreeMap::from([("fixture".into(), "fixture-only".into())])),
    };
    for header in [None, Some("Basic fixture-only"), Some("Bearer wrong")] {
        let mut request = Request::builder().uri("/api/v1/memories");
        if let Some(header) = header {
            request = request.header("authorization", header);
        }
        let response = router(state.clone())
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let value: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(value["error"]["code"], "unauthorized");
        assert_eq!(value["error"]["details"], json!({}));
    }
    let response = router(state)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
