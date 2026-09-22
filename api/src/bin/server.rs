use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let bind = config.bind_addr.clone();
    let run_seed = config.run_seed;

    let state = backline::build_state(config).await?;

    // `docker compose up` on a clean machine must produce a working
    // application with the demo data (§18).
    if run_seed {
        if let Err(e) = backline::seed::run(&state.db).await {
            tracing::error!(error = %e, "seeding");
        }
    }

    // Daily render purge (§15), primed at startup.
    backline::services::jobs::enqueue(
        &state.db,
        "purge_renders",
        chrono::Utc::now() + chrono::Duration::minutes(5),
        serde_json::json!({}),
        Some("purge:boot".into()),
    )
    .await
    .ok();

    // The job loop runs in the same process: one less service to operate, and
    // reminders go out even if nobody opens the app.
    tokio::spawn(backline::services::scheduler::run_forever(state.clone()));

    let app = backline::routes::router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "backline api");
    axum::serve(listener, app).await?;
    Ok(())
}
