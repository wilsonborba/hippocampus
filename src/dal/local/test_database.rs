use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;

/// Development and all tests stay on the isolated copy.
pub async fn connect() -> Result<PgPool, Box<dyn std::error::Error>> {
    connect_target(false).await
}
/// Only an explicit production launch selects the production database.
pub async fn connect_target(production: bool) -> Result<PgPool, Box<dyn std::error::Error>> {
    let (key, database) = if production {
        ("HIPPOCAMPUS_DATABASE_URL", "hippocampus")
    } else {
        ("HIPPOCAMPUS_TEST_DATABASE_URL", "hippocampus_v2_test")
    };
    let url = std::env::var(key).map_err(|_| "Database secret is missing")?;
    let url = url.replacen("postgresql+psycopg://", "postgresql://", 1);
    let options = PgConnectOptions::from_str(&url).map_err(|_| "Database address is invalid")?;
    if options.get_database() != Some(database) || options.get_username() != "appuser" {
        return Err("Database name or role is not approved".into());
    }
    PgPoolOptions::new()
        .max_connections(crate::core::config::Config::default().database_connections)
        .connect_with(options)
        .await
        .map_err(|_| "Database connection failed".into())
}
