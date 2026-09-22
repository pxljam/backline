use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let bind = config.bind_addr.clone();
    let run_seed = config.run_seed;

    let state = backline::build_state(config).await?;

    // `docker compose up` sur une machine vierge doit donner une application
    // fonctionnelle avec les donnees de demonstration (§18).
    if run_seed {
        if let Err(e) = backline::seed::run(&state.db).await {
            tracing::error!(error = %e, "amorcage");
        }
    }

    // Purge quotidienne des rendus (§15), amorcee au demarrage.
    backline::services::jobs::enqueue(
        &state.db,
        "purge_renders",
        chrono::Utc::now() + chrono::Duration::minutes(5),
        serde_json::json!({}),
        Some("purge:boot".into()),
    )
    .await
    .ok();

    // La boucle de jobs tourne dans le meme processus : un service de moins a
    // exploiter, et les rappels partent meme si personne n'ouvre l'app.
    tokio::spawn(backline::services::scheduler::run_forever(state.clone()));

    let app = backline::routes::router(state);
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "backline api");
    axum::serve(listener, app).await?;
    Ok(())
}
