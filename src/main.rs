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
    let config = Config::default();
    let pool = test_database::connect().await?;
    let keys = std::env::var("HIPPOCAMPUS_API_KEYS")
        .ok()
        .map(|v| serde_json::from_str::<BTreeMap<String, String>>(&v))
        .transpose()?
        .unwrap_or_default();
    let state = AppState {
        filestore: std::sync::Arc::new(
            hippocampus::dal::remote::files::FsmClient::from_environment(),
        ),
        service: MemoryService {
            store: Store { pool },
        },
        documents: Documents::from_environment().await,
        api_keys: Arc::new(keys),
    };
    let listener = tokio::net::TcpListener::bind(config.listen).await?;
    tracing::info!(address=%config.listen,"Development server started");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
