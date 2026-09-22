use anyhow::Result;

/// Service du bot Telegram (§13). Separe de l'API : une panne de Telegram ne
/// doit ni ralentir ni faire tomber les requetes web.
#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let state = backline::build_state(config).await?;
    backline::services::bot::run_forever(state).await;
    Ok(())
}
