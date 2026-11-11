#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let pool = hippocampus::dal::local::test_database::connect().await?;
    sqlx::migrate!().run(&pool).await?;
    println!("Test database migrations passed");
    Ok(())
}
