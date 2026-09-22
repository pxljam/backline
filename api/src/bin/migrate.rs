use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let db = backline::db::connect(&config.database_url).await?;
    backline::db::migrate(&db).await?;
    println!("migrations appliquees");
    Ok(())
}
