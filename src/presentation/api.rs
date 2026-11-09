use crate::presentation::validation::{ApiJson, ApiQuery};
use crate::{
    dal::{local::store::Table, remote::documents::Documents},
    domain::{
        models::{api::*, error::Error},
        services::memories::{Filters, MemoryService, projection},
    },
};
use axum::{
    Json, Router,
    extract::{Path, Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct AppState {
    pub service: MemoryService,
    pub documents: Documents,
    pub api_keys: Arc<BTreeMap<String, String>>,
}
fn data(value: Value) -> Json<Value> {
    Json(json!({"data":value,"pagination":null}))
}
fn out(value: Value, table: Table) -> Json<Value> {
    data(projection(&value, table))
}
fn outs(values: Vec<Value>, table: Table) -> Json<Value> {
    data(Value::Array(
        values.iter().map(|v| projection(v, table)).collect(),
    ))
}

pub fn router(state: AppState) -> Router {
    let private = Router::new()
        .route("/api/v1/consolidation", post(consolidate))
        .route(
            "/api/v1/memories",
            get(list)
                .post(create)
                .on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/api/v1/memories/{id}",
            get(read)
                .on(axum::routing::MethodFilter::HEAD, wrong_method)
                .patch(update),
        )
        .route("/api/v1/memories/{id}/tags", post(tag))
        .route(
            "/api/v1/memories/{id}/tags/{tag}",
            axum::routing::delete(untag),
        )
        .route("/api/v1/memories/{id}/entities", post(entity))
        .route("/api/v1/memories/{id}/resources", post(resource_link))
        .route("/api/v1/memories/{id}/relationships", post(relationship))
        .route(
            "/api/v1/memories/{id}/neighbors",
            get(neighbors).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route("/api/v1/memories/{id}/reinforce", post(reinforce))
        .route("/api/v1/memories/{id}/supersede", post(supersede))
        .route("/api/v1/memories/{id}/correct", post(correct))
        .route("/api/v1/memories/{id}/archive", post(archive))
        .route("/api/v1/memories/{id}/forget", post(forget))
        .route("/api/v1/memories/{id}/restore", post(restore))
        .route(
            "/api/v1/memories/{id}/events",
            get(events).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/api/v1/memories/{id}/document",
            get(document).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/api/v1/memories/{id}/graph",
            get(graph).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route("/api/v1/resources", post(register_resource))
        .route(
            "/api/v1/resources/{id}",
            get(read_resource).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/api/v1/entities/{id}",
            get(read_entity).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route_layer(middleware::from_fn_with_state(state.clone(), auth));
    Router::new()
        .route(
            "/docs",
            get(scalar_docs).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/docs/scalar",
            get(scalar_docs).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route("/openapi.json", get(openapi))
        .route(
            "/health",
            get(|| async { Json(json!({"status":"ok"})) })
                .on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .route(
            "/ready",
            get(ready).on(axum::routing::MethodFilter::HEAD, wrong_method),
        )
        .merge(private)
        .layer(axum::extract::DefaultBodyLimit::disable())
        .fallback(not_found)
        .method_not_allowed_fallback(wrong_method)
        .layer(middleware::from_fn(cors))
        .with_state(state)
}
async fn auth(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, Error> {
    if !state.api_keys.is_empty() {
        let header = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|s| s.to_str().ok());
        let Some(header) = header.filter(|s| s.to_lowercase().starts_with("bearer ")) else {
            return Err(Error::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "missing or malformed Authorization header",
            ));
        };
        let token = header.split_once(' ').expect("bearer prefix").1.trim();
        if !state
            .api_keys
            .values()
            .any(|key| constant_time_equal(key.as_bytes(), token.as_bytes()))
        {
            return Err(Error::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "invalid API key",
            ));
        }
    }
    Ok(next.run(request).await)
}
fn constant_time_equal(a: &[u8], b: &[u8]) -> bool {
    let mut diff = a.len() ^ b.len();
    for i in 0..a.len().max(b.len()) {
        diff |= (a.get(i).copied().unwrap_or(0) ^ b.get(i).copied().unwrap_or(0)) as usize;
    }
    diff == 0
}
pub async fn ready(State(s): State<AppState>) -> Json<Value> {
    let pg = s.service.store.available().await;
    let couch = if std::env::var("HIPPOCAMPUS_COUCHDB_URL").is_ok_and(|v| !v.is_empty()) {
        if s.documents.available().await {
            "ok"
        } else {
            "degraded"
        }
    } else {
        "unconfigured"
    };
    Json(
        json!({"status":if pg{"ready"}else{"not_ready"},"dependencies":{"postgresql":if pg{"ok"}else{"unavailable"},"couchdb":couch,"filestore":if std::env::var("HIPPOCAMPUS_FILESTORE_URL").is_ok_and(|v| !v.is_empty()){"ok"}else{"unconfigured"},"embedding_provider":if crate::dal::remote::embeddings::configured(&crate::core::config::Config::default().embedding).name()=="none"{"none"}else{"ollama"}}}),
    )
}
async fn create(
    State(s): State<AppState>,
    ApiJson(body): ApiJson<MemoryCreateRequest>,
) -> Result<impl IntoResponse, Error> {
    Ok((
        StatusCode::CREATED,
        out(s.service.create(body).await?, Table::Memory),
    ))
}
async fn list(State(s): State<AppState>, ApiQuery(f): ApiQuery) -> Result<Json<Value>, Error> {
    Ok(outs(s.service.list(&f).await?, Table::Memory))
}
async fn read(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiQuery(f): ApiQuery,
) -> Result<Json<Value>, Error> {
    Ok(out(s.service.get(&id, &f, true).await?, Table::Memory))
}
async fn update(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<MemoryUpdateRequest>,
) -> Result<Json<Value>, Error> {
    Ok(out(s.service.update(&id, body).await?, Table::Memory))
}
async fn tag(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<TagRequest>,
) -> Result<Json<Value>, Error> {
    Ok(outs(s.service.tag(&id, body).await?, Table::Tag))
}
async fn untag(
    State(s): State<AppState>,
    Path((id, tag)): Path<(String, String)>,
) -> Result<StatusCode, Error> {
    s.service.untag(&id, &tag).await?;
    Ok(StatusCode::NO_CONTENT)
}
async fn entity(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<EntityAssociateRequest>,
) -> Result<Json<Value>, Error> {
    Ok(out(
        s.service.associate_entity(&id, body, None).await?,
        Table::Entity,
    ))
}
async fn resource_link(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ResourceLinkRequest>,
) -> Result<Json<Value>, Error> {
    Ok(out(
        s.service.associate_resource(&id, body).await?,
        Table::Resource,
    ))
}
async fn relationship(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<RelationshipCreateRequest>,
) -> Result<impl IntoResponse, Error> {
    Ok((
        StatusCode::CREATED,
        out(
            s.service.relationship(&id, body, true).await?,
            Table::Relationship,
        ),
    ))
}
async fn neighbors(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiQuery(f): ApiQuery,
) -> Result<Json<Value>, Error> {
    let depth = f.get("depth").and_then(|s| s.parse().ok()).unwrap_or(1);
    Ok(outs(
        s.service
            .neighbors(&id, f.get("relation_type").map(String::as_str), depth)
            .await?,
        Table::Relationship,
    ))
}
async fn reinforce(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ReinforceRequest>,
) -> Result<Json<Value>, Error> {
    Ok(out(s.service.reinforce(&id, body).await?, Table::Memory))
}
async fn supersede(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<SupersedeRequest>,
) -> Result<Json<Value>, Error> {
    s.service.replace(&id, &body.new_memory_id, false).await?;
    Ok(data(json!({"memory_id":id,"status":"superseded"})))
}
async fn correct(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<CorrectRequest>,
) -> Result<Json<Value>, Error> {
    s.service.replace(&id, &body.new_memory_id, true).await?;
    Ok(data(json!({"memory_id":id,"status":"superseded"})))
}
async fn archive(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, Error> {
    s.service
        .status(&id, "archived", json!({}), &Filters::new())
        .await?;
    Ok(data(json!({"memory_id":id,"status":"archived"})))
}
async fn forget(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiQuery(f): ApiQuery,
    ApiJson(body): ApiJson<ForgetRequest>,
) -> Result<Json<Value>, Error> {
    s.service
        .status(&id, "forgotten", json!({"reason":body.reason}), &f)
        .await?;
    Ok(data(json!({"memory_id":id,"status":"forgotten"})))
}
async fn restore(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, Error> {
    Ok(out(
        s.service
            .status(&id, "active", json!({}), &Filters::new())
            .await?,
        Table::Memory,
    ))
}
async fn events(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, Error> {
    s.service.get(&id, &Filters::new(), false).await?;
    Ok(outs(s.service.store.events(&id).await?, Table::Event))
}
async fn document(State(s): State<AppState>, Path(id): Path<String>) -> Result<Json<Value>, Error> {
    s.service.get(&id, &Filters::new(), false).await?;
    Ok(data(s.documents.get(&id).await.unwrap_or(Value::Null)))
}
async fn register_resource(
    State(s): State<AppState>,
    ApiJson(body): ApiJson<ResourceRegisterRequest>,
) -> Result<impl IntoResponse, Error> {
    Ok((
        StatusCode::CREATED,
        out(s.service.register_resource(body).await?, Table::Resource),
    ))
}
async fn read_resource(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(out(
        s.service
            .store
            .get(Table::Resource, &id)
            .await?
            .ok_or_else(|| Error::not_found("resource", &id))?,
        Table::Resource,
    ))
}
async fn read_entity(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, Error> {
    Ok(out(
        s.service
            .store
            .get(Table::Entity, &id)
            .await?
            .ok_or_else(|| Error::not_found("entity", &id))?,
        Table::Entity,
    ))
}

async fn graph(
    State(s): State<AppState>,
    Path(id): Path<String>,
    ApiQuery(f): ApiQuery,
) -> Result<Response, Error> {
    let graph = s.service.graph(&id, &f).await?;
    crate::presentation::graph::render(graph, f.get("format").map(String::as_str).unwrap_or("json"))
        .await
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(json!({"error":{"code":self.name,"message":self.message,"details":{}}})),
        )
            .into_response()
    }
}

async fn not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"detail":"Not Found"}))).into_response()
}
async fn wrong_method() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        Json(json!({"detail":"Method Not Allowed"})),
    )
        .into_response()
}
async fn cors(request: Request, next: Next) -> Response {
    let config = crate::core::config::Config::default();
    let wildcard = config.cors_allow_origins.iter().any(|x| x == "*");
    let origin = request.headers().get("origin").cloned().filter(|o| {
        wildcard
            || o.to_str()
                .is_ok_and(|o| config.cors_allow_origins.iter().any(|x| x == o))
    });
    if let Some(preflight_origin) = origin.clone()
        && request.method() == axum::http::Method::OPTIONS
        && request
            .headers()
            .contains_key("access-control-request-method")
    {
        let method = request
            .headers()
            .get("access-control-request-method")
            .unwrap()
            .to_str()
            .unwrap_or("");
        let allowed =
            ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"].contains(&method);
        let mut response = (
            if allowed {
                StatusCode::OK
            } else {
                StatusCode::BAD_REQUEST
            },
            if allowed {
                "OK"
            } else {
                "Disallowed CORS method"
            },
        )
            .into_response();
        for (key, val) in [
            ("access-control-allow-origin", preflight_origin),
            (
                "access-control-allow-credentials",
                header::HeaderValue::from_static("true"),
            ),
            (
                "access-control-allow-methods",
                header::HeaderValue::from_static("DELETE, GET, HEAD, OPTIONS, PATCH, POST, PUT"),
            ),
            (
                "access-control-max-age",
                header::HeaderValue::from_str(&config.cors_max_age_seconds.to_string())
                    .expect("max age"),
            ),
            ("vary", header::HeaderValue::from_static("Origin")),
        ] {
            response
                .headers_mut()
                .insert(header::HeaderName::from_static(key), val);
        }
        if let Some(headers) = request.headers().get("access-control-request-headers") {
            response.headers_mut().insert(
                header::HeaderName::from_static("access-control-allow-headers"),
                headers.clone(),
            );
        }
        return response;
    }
    let cookie = request.headers().contains_key(header::COOKIE);
    let mut response = next.run(request).await;
    if let Some(origin) = origin {
        response.headers_mut().insert(
            header::HeaderName::from_static("access-control-allow-origin"),
            if cookie {
                origin
            } else {
                header::HeaderValue::from_static("*")
            },
        );
        response.headers_mut().insert(
            header::HeaderName::from_static("access-control-allow-credentials"),
            header::HeaderValue::from_static("true"),
        );
        if cookie {
            response
                .headers_mut()
                .append(header::VARY, header::HeaderValue::from_static("Origin"));
        }
    }
    response
}

async fn consolidate(
    State(s): State<AppState>,
    ApiJson(body): ApiJson<ConsolidationRequest>,
) -> Result<Json<Value>, Error> {
    Ok(data(s.service.consolidate(body).await?))
}

async fn scalar_docs() -> axum::response::Html<&'static str> {
    axum::response::Html(include_str!("scalar.html"))
}
async fn openapi() -> Json<Value> {
    Json(serde_json::from_str(include_str!("openapi.json")).expect("OpenAPI contract"))
}
