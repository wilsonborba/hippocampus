use hippocampus::dal::local::test_database;
use serde_json::json;
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await?;
    let count = || async {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM memory")
            .fetch_one(&pool)
            .await
    };
    let before = count().await?;
    let uuid_rows:i64=sqlx::query_scalar("SELECT count(*) FROM memory WHERE workspace_id ~* '^([0-9a-f]{32}|[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12})$'").fetch_one(&pool).await?;
    let legacy_before:i64=sqlx::query_scalar("SELECT count(*) FROM memory WHERE app_id='legacy' AND user_id IS NULL AND workspace_id IN ('asodya','work','university','default')").fetch_one(&pool).await?;
    sqlx::migrate!().run(&pool).await?;
    let after = count().await?;
    let legacy_after:i64=sqlx::query_scalar("SELECT count(*) FROM memory WHERE app_id='legacy' AND user_id IS NULL AND workspace_id IN ('asodya','work','university','default')").fetch_one(&pool).await?;
    assert_eq!(before - after, uuid_rows);
    assert_eq!(legacy_before, legacy_after);
    println!(
        "{}",
        json!({"before":before,"uuid_workspace_rows":uuid_rows,"after":after,"legacy_before":legacy_before,"legacy_after":legacy_after})
    );
    Ok(())
}
