// sqlx queries are written as typed tuples rather than dedicated structs: on a
// domain this size, one struct per SELECT would cost more to read than it
// returns. The integration tests run against a real database, which covers what
// the `query!` macro would cover.
#![allow(clippy::type_complexity)]

//! Backline — domain API.
//!
//! The full specification lives in `PRD.md` at the root of the repository. The
//! `§n` references in the comments point into it.
//!
//! Two structural invariants run through the whole codebase:
//!
//! 1. **Isolation** — access to a collective's data always goes through a
//!    [`scope::CollectiveScope`], which cannot be built without membership.
//! 2. **One layout implementation** — the JSON description of visuals lives in
//!    `layout/`, on the Node side; the API only ever transports it.

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod extract;
pub mod routes;
pub mod scope;
pub mod seed;
pub mod services;
pub mod state;

pub use config::Config;
pub use error::{AppError, AppResult};
pub use state::AppState;

use anyhow::Result;
use std::sync::Arc;

/// Builds the application state: database, storage, Telegram channel.
pub async fn build_state(config: Config) -> Result<AppState> {
    let db = db::connect(&config.database_url).await?;
    if config.run_migrations {
        db::migrate(&db).await?;
    }

    let storage = Arc::new(services::storage::Storage::new(&config.s3)?);
    if let Err(e) = storage.ensure_bucket().await {
        tracing::warn!(error = %e, "S3 bucket unavailable at startup");
    }

    let telegram: Arc<dyn services::telegram::Telegram> = match &config.telegram_bot_token {
        Some(token) => Arc::new(services::telegram::HttpTelegram::new(token.clone())),
        None => {
            tracing::warn!("TELEGRAM_BOT_TOKEN missing — notifications will only be logged");
            Arc::new(services::telegram::LoggingTelegram)
        }
    };

    Ok(AppState {
        db,
        config: Arc::new(config),
        storage,
        telegram,
    })
}

pub fn init_tracing() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,backline=debug,sqlx=warn"));
    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_target(true))
        .try_init();
}
