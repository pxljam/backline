use anyhow::Result;

/// Telegram bot service (§13). Separate from the API: a Telegram outage must
/// neither slow down nor bring down web requests.
#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let state = backline::build_state(config).await?;
    backline::services::bot::run_forever(state).await;
    Ok(())
}
