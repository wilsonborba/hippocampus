use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};

/// Private values stay in .env. These settings are safe to commit.
#[derive(Debug, Clone)]
pub struct Config {
    pub listen: SocketAddr,
    pub document_database: String,
    pub database_connections: u32,
    pub document_timeout: Duration,
    pub graph_render_timeout: Duration,
    pub graph_max_depth: usize,
    pub graph_cluster_threshold: usize,
    pub graph_default_max_nodes: usize,
    pub list_default_limit: i64,
    pub cors_max_age_seconds: u32,
    pub cors_allow_origins: Vec<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            listen: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 18001),
            document_database: "hippocampus_v2_test".into(),
            database_connections: 5,
            document_timeout: Duration::from_secs(10),
            graph_render_timeout: Duration::from_secs(10),
            graph_max_depth: 2,
            graph_cluster_threshold: 40,
            graph_default_max_nodes: 300,
            list_default_limit: 20,
            cors_max_age_seconds: 600,
            cors_allow_origins: vec!["*".into()],
        }
    }
}
