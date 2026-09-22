use anyhow::Result;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Versioned SQL migrations, embedded in the binary: no schema change ever
/// happens outside a migration (§21, conventions).
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

pub async fn migrate(pool: &PgPool) -> Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}
