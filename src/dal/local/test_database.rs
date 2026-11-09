use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};
use std::str::FromStr;

/// This night-run binary must never open the production database.
pub async fn connect() -> Result<PgPool, Box<dyn std::error::Error>> {
    let url = std::env::var("HIPPOCAMPUS_TEST_DATABASE_URL")?;
    let options = PgConnectOptions::from_str(&url)?;
    if options.get_database() != Some("hippocampus_v2_test") {
        return Err("Only hippocampus_v2_test is allowed before cutover".into());
    }
    Ok(PgPoolOptions::new()
        .max_connections(crate::core::config::Config::default().database_connections)
        .connect_with(options)
        .await?)
}
