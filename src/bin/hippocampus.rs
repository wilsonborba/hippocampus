use clap::Parser;
use hippocampus::{
    dal::{
        local::{store::Store, test_database},
        remote::documents::Documents,
    },
    domain::services::memories::MemoryService,
    presentation::{
        api::AppState,
        cli::{Cli, execute},
    },
};
#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    dotenvy::dotenv().ok();
    if hippocampus::core::logs::init(true, cli.verbose()).is_err() {
        eprintln!("Error: The log file could not open");
        std::process::exit(1);
    }
    let result = async {
        let config = if cli.production() {
            hippocampus::core::config::Config::production()
        } else {
            Default::default()
        };
        let pool = test_database::connect_target(cli.production())
            .await
            .map_err(|_| "The database connection failed".to_owned())?;
        let documents = Documents::from_config(&config).await;
        if cli.production() && !documents.persistent() {
            return Err("The production document database is unavailable".to_owned());
        }
        let state = AppState {
            embedding: hippocampus::dal::remote::embeddings::configured(&Default::default()),
            filestore: std::sync::Arc::new(
                hippocampus::dal::remote::files::FsmClient::from_environment(),
            ),
            service: MemoryService {
                store: Store { pool },
            },
            documents,
            api_keys: Default::default(),
        };
        execute(cli, state).await.map_err(|e| e.message)
    }
    .await;
    if let Err(message) = result {
        eprintln!("Error: {message}");
        std::process::exit(1);
    }
}
