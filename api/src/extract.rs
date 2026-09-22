//! Axum extractors: the authenticated user, and nothing more. Permissions are
//! proven afterwards through [`crate::scope::CollectiveScope`].

use crate::auth::session::{actor_from_token, SESSION_COOKIE};
use crate::error::AppError;
use crate::scope::Actor;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

pub struct Auth(pub Actor);

#[async_trait::async_trait]
impl FromRequestParts<AppState> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer(parts)
            .or_else(|| cookie(parts))
            .ok_or(AppError::Unauthorized)?;
        let actor = actor_from_token(&state.db, &token).await?;
        Ok(Auth(actor))
    }
}

fn bearer(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_owned)
}

fn cookie(parts: &Parts) -> Option<String> {
    let raw = parts
        .headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?;
    raw.split(';')
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == SESSION_COOKIE)
        .map(|(_, v)| v.to_owned())
}
