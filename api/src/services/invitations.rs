//! Single-use invitation links (§3).
//!
//! The only way an access is created: an admin creates the person, the link
//! turns them into someone who can sign in. There is no public sign-up, so this
//! is the one door and it is worth having in a single place.

use crate::error::AppResult;
use chrono::{Duration, Utc};
use uuid::Uuid;

/// Long enough that a link sent on a Monday still works the following month,
/// short enough that a forwarded message stops being a key.
pub const VALIDITY_DAYS: i64 = 30;

/// Mints a code for `user_id`. Takes any executor, so it works inside the
/// transaction that creates the user as well as on its own.
pub async fn issue<'e, E>(db: E, user_id: Uuid, created_by: Option<Uuid>) -> AppResult<String>
where
    E: sqlx::PgExecutor<'e>,
{
    let code = crate::auth::session::random_token();
    sqlx::query(
        "INSERT INTO invitations (user_id, code, created_by, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(&code)
    .bind(created_by)
    .bind(Utc::now() + Duration::days(VALIDITY_DAYS))
    .execute(db)
    .await?;
    Ok(code)
}

/// Kills the links already sent. Called before issuing a replacement, so a
/// person never holds two working keys.
pub async fn expire_pending<'e, E>(db: E, user_id: Uuid) -> AppResult<()>
where
    E: sqlx::PgExecutor<'e>,
{
    sqlx::query("UPDATE invitations SET expires_at = now() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(db)
        .await?;
    Ok(())
}

/// The address the person opens. Built from `PUBLIC_BASE_URL`, which is why
/// that variable must carry the real scheme and host in production.
pub fn url(public_base_url: &str, code: &str) -> String {
    format!("{public_base_url}/invitation/{code}")
}
