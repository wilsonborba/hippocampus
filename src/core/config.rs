use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

/// Private values stay in .env. These settings are safe to commit.
#[derive(Debug, Clone)]
pub struct Config {
    pub embedding: EmbeddingConfig,
    pub listen: SocketAddr,
    pub document_database: String,
    pub database_connections: u32,
    pub document_timeout: Duration,
    pub graph_render_timeout: Duration,
    pub graph_max_depth: usize,
    pub graph_cluster_threshold: usize,
    pub graph_default_max_nodes: usize,
    pub list_default_limit: i64,
    pub scheduler_enabled: bool,
    pub expiration_interval: Duration,
    pub document_sync_interval: Duration,
    pub reconciliation_interval: Duration,
    pub expiration_batch_limit: i64,
    pub document_batch_limit: i64,
    pub reconciliation_batch_limit: i64,
    pub consolidation_max_candidates: usize,
    pub cli_app_id: String,
    pub log_file: std::path::PathBuf,
    pub log_max_bytes: u64,
    pub log_backups: usize,
    pub cors_max_age_seconds: u32,
    pub cors_allow_origins: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            embedding: EmbeddingConfig::default(),
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 18001),
            document_database: "hippocampus_v2_test".into(),
            database_connections: 5,
            document_timeout: Duration::from_secs(10),
            graph_render_timeout: Duration::from_secs(10),
            graph_max_depth: 2,
            graph_cluster_threshold: 40,
            graph_default_max_nodes: 300,
            list_default_limit: 20,
            scheduler_enabled: true,
            expiration_interval: Duration::from_secs(300),
            document_sync_interval: Duration::from_secs(120),
            reconciliation_interval: Duration::from_secs(1800),
            expiration_batch_limit: 200,
            document_batch_limit: 50,
            reconciliation_batch_limit: 200,
            consolidation_max_candidates: 50,
            cli_app_id: "hippocampus".into(),
            log_file: "var/hippocampus.log".into(),
            log_max_bytes: 5 * 1024 * 1024,
            log_backups: 3,
            cors_max_age_seconds: 600,
            cors_allow_origins: vec!["*".into()],
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum EmbeddingBackend {
    None,
    Ollama,
}
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    pub backend: EmbeddingBackend,
    pub model: Option<String>,
    pub base_url: String,
    pub timeout: Duration,
    pub batch_limit: i64,
    pub interval: Duration,
}
impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            backend: EmbeddingBackend::None,
            model: None,
            base_url: "http://192.168.1.104:11434".into(),
            timeout: Duration::from_secs(30),
            batch_limit: 50,
            interval: Duration::from_secs(60),
        }
    }
}
