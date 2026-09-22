//! Assemblage du routeur. Tout ce qui touche a un collectif vit sous
//! `/api/collectives/:collective_id/…` — l'identifiant est dans l'URL parce
//! qu'un utilisateur appartient a plusieurs collectifs et bascule de l'un a
//! l'autre (§3, §13 `/collectif`).

use crate::state::AppState;
use axum::routing::get;
use axum::Router;
use tower_http::trace::TraceLayer;

pub mod auth;
pub mod calendar;
pub mod collectives;
pub mod comms;
pub mod dashboard;
pub mod events;
pub mod instance;
pub mod notifications;
pub mod opportunities;
pub mod riders;
pub mod studio;
pub mod venues;
pub mod video;

pub fn router(state: AppState) -> Router {
    let collective = Router::new()
        .merge(collectives::router())
        .nest("/venues", venues::router())
        .nest("/opportunities", opportunities::router())
        .nest("/events", events::router())
        .nest("/calendar", calendar::router())
        .nest("/studio", studio::router())
        .merge(riders::router())
        .merge(comms::router())
        .merge(dashboard::router())
        .merge(video::router());

    let api = Router::new()
        .nest("/auth", auth::router())
        .route("/me", get(auth::me))
        .nest("/instance", instance::router())
        .nest("/notifications", notifications::router())
        .nest("/render", video::machine_router())
        .nest("/collectives/:collective_id", collective);

    Router::new()
        .route("/health", get(health))
        .nest("/api", api.route("/config", get(public_config)))
        .nest("/ical", calendar::public_router())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> &'static str {
    "ok"
}

/// Ce que l'interface doit savoir **avant** toute connexion : y a-t-il un bot
/// Telegram sur cette instance, et sous quel nom ? Rien de secret ici.
async fn public_config(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "telegram_bot_username": state.config.telegram_bot_username,
        "telegram_enabled": state.config.telegram_bot_token.is_some(),
        "public_base_url": state.config.public_base_url,
    }))
}
