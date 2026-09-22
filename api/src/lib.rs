// Les requetes sqlx sont ecrites en tuples typees plutot qu'en structs
// dediees : sur un domaine de cette taille, une struct par SELECT couterait
// plus en lecture qu'elle ne rapporte. Les tests d'integration tournent sur
// une vraie base, ce qui couvre ce que la macro `query!` couvrirait.
#![allow(clippy::type_complexity)]

//! Backline — API metier.
//!
//! La specification complete est dans `PRD.md` a la racine du depot. Les
//! references `§n` dans les commentaires y renvoient.
//!
//! Deux invariants structurels traversent tout le code :
//!
//! 1. **Cloisonnement** — l'acces aux donnees d'un collectif passe toujours par
//!    un [`scope::CollectiveScope`], impossible a fabriquer sans appartenance.
//! 2. **Une seule implementation de mise en page** — la description JSON des
//!    visuels vit dans `layout/`, cote Node ; l'API ne fait que la transporter.

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

/// Construit l'etat applicatif : base, stockage, canal Telegram.
pub async fn build_state(config: Config) -> Result<AppState> {
    let db = db::connect(&config.database_url).await?;
    if config.run_migrations {
        db::migrate(&db).await?;
    }

    let storage = Arc::new(services::storage::Storage::new(&config.s3)?);
    if let Err(e) = storage.ensure_bucket().await {
        tracing::warn!(error = %e, "bucket S3 indisponible au demarrage");
    }

    let telegram: Arc<dyn services::telegram::Telegram> = match &config.telegram_bot_token {
        Some(token) => Arc::new(services::telegram::HttpTelegram::new(token.clone())),
        None => {
            tracing::warn!("TELEGRAM_BOT_TOKEN absent — notifications journalisees seulement");
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
