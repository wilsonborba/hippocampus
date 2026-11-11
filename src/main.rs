use hippocampus::{
    core::config::Config,
    dal::{
        local::{store::Store, test_database},
        remote::documents::Documents,
    },
    domain::services::memories::MemoryService,
    presentation::api::{AppState, router},
};
use std::{collections::BTreeMap, sync::Arc};
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    hippocampus::core::logs::init(false, false)?;
    let production = match std::env::args().skip(1).collect::<Vec<_>>().as_slice() {
        [] => false,
        [mode] if mode == "--production" => true,
        _ => return Err("Use hippocampus-api [--production]".into()),
    };
    let config = if production {
        Config::production()
    } else {
        Config::default()
    };
    let pool = test_database::connect_target(production).await?;
    let documents = Documents::from_config(&config).await;
    if production && !documents.persistent() {
        return Err("The production document database is unavailable".into());
    }
    let keys = std::env::var("HIPPOCAMPUS_API_KEYS")
        .ok()
        .map(|v| serde_json::from_str::<BTreeMap<String, String>>(&v))
        .transpose()?
        .unwrap_or_default();
    let state = AppState {
        embedding: hippocampus::dal::remote::embeddings::configured(&Default::default()),
        filestore: std::sync::Arc::new(
            hippocampus::dal::remote::files::FsmClient::from_environment(),
        ),
        service: MemoryService {
            store: Store { pool },
        },
        documents,
        api_keys: Arc::new(keys),
    };
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(address=%config.listen,"Server started");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
