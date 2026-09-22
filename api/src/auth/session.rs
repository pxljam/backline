use crate::error::{AppError, AppResult};
use chrono::{Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub const SESSION_COOKIE: &str = "bl_session";
const SESSION_DAYS: i64 = 60;

/// The token is never stored in clear: only its hash goes to the database,
/// exactly like a password.
pub fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub fn random_token() -> String {
    let mut buf = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut buf);
    hex::encode(buf)
}

pub async fn create_session(db: &PgPool, user_id: Uuid) -> AppResult<String> {
    let token = random_token();
    sqlx::query("INSERT INTO sessions (user_id, token_hash, expires_at) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(hash_token(&token))
        .bind(Utc::now() + Duration::days(SESSION_DAYS))
        .execute(db)
        .await?;
    Ok(token)
}

pub async fn destroy_session(db: &PgPool, token: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(hash_token(token))
        .execute(db)
        .await?;
    Ok(())
}

pub async fn actor_from_token(db: &PgPool, token: &str) -> AppResult<crate::scope::Actor> {
    let row: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT u.id, u.is_instance_admin
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = $1 AND s.expires_at > now()",
    )
    .bind(hash_token(token))
    .fetch_optional(db)
    .await?;

    row.map(|(user_id, is_instance_admin)| crate::scope::Actor {
        user_id,
        is_instance_admin,
    })
    .ok_or(AppError::Unauthorized)
}

pub fn cookie_header(token: &str, secure: bool) -> String {
    let flags = if secure { "; Secure" } else { "" };
    format!(
        "{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{flags}",
        SESSION_DAYS * 86_400
    )
}

pub fn clear_cookie_header() -> String {
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0")
}
