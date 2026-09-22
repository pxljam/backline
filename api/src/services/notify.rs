//! Web notification centre + Telegram delivery (§13, §14).
//!
//! An HTTP request never talks to Telegram: it **drops off** a notification,
//! which a job delivers. A slow network or a broken Telegram API therefore
//! cannot make a domain action fail.

use crate::error::AppResult;
use crate::state::AppState;
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

pub struct Notice<'a> {
    pub user_id: Uuid,
    pub collective_id: Option<Uuid>,
    pub kind: &'a str,
    pub title: String,
    pub body: String,
    pub payload: Value,
}

pub async fn push(db: &PgPool, n: Notice<'_>) -> AppResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO notifications (user_id, collective_id, kind, title, body, payload)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(n.user_id)
    .bind(n.collective_id)
    .bind(n.kind)
    .bind(&n.title)
    .bind(&n.body)
    .bind(&n.payload)
    .fetch_one(db)
    .await?;
    Ok(id)
}

/// Respects quiet hours and per-type opt-outs (§13).
pub fn is_muted(
    quiet_from: Option<i16>,
    quiet_to: Option<i16>,
    opt_out: &[String],
    kind: &str,
    hour: u32,
) -> bool {
    if opt_out.iter().any(|k| k == kind) {
        return true;
    }
    match (quiet_from, quiet_to) {
        (Some(from), Some(to)) => {
            let (from, to, hour) = (from as u32, to as u32, hour);
            if from <= to {
                hour >= from && hour < to
            } else {
                // range straddling midnight: 22:00 -> 08:00
                hour >= from || hour < to
            }
        }
        _ => false,
    }
}

/// Delivers pending notifications to Telegram. Critical alerts ("we are live",
/// admin alerts) cut through quiet hours.
pub async fn deliver_pending(state: &AppState, limit: i64) -> AppResult<usize> {
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        String,
        Value,
        Option<i64>,
        Option<i16>,
        Option<i16>,
        Vec<String>,
    )> = sqlx::query_as(
        "SELECT n.id, n.user_id, n.kind, n.title, n.body, n.payload,
                    u.telegram_id, u.notif_quiet_from, u.notif_quiet_to, u.notif_opt_out
             FROM notifications n JOIN users u ON u.id = n.user_id
             WHERE n.delivered_telegram = FALSE
             ORDER BY n.created_at
             LIMIT $1",
    )
    .bind(limit)
    .fetch_all(&state.db)
    .await?;

    let hour = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Paris);
    let hour = chrono::Timelike::hour(&hour);
    let mut sent = 0usize;

    for (id, _user, kind, title, body, payload, tg, qf, qt, opt_out) in rows {
        let critical = matches!(
            kind.as_str(),
            "stream_live" | "admin_alert" | "publication_missed"
        );
        let muted = !critical && is_muted(qf, qt, &opt_out, &kind, hour);

        if let (Some(chat_id), false) = (tg, muted) {
            let text = if body.is_empty() {
                title.clone()
            } else {
                format!("<b>{title}</b>\n{body}")
            };
            // A notification that calls for an action goes out with its
            // button: two taps, not a round trip through the web (§13).
            if let Err(e) = state
                .telegram
                .send(crate::services::telegram::OutgoingMessage {
                    chat_id,
                    text,
                    keyboard: crate::services::bot::keyboard_for(&kind, &payload),
                    photo_url: None,
                })
                .await
            {
                tracing::warn!(error = %e, "telegram delivery failed");
                continue;
            }
            sent += 1;
        }
        // A silenced notification stays visible in the web centre: mark it
        // delivered so it is not replayed forever.
        sqlx::query("UPDATE notifications SET delivered_telegram = TRUE WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
    }
    Ok(sent)
}

/// Every member of a collective.
pub async fn collective_member_ids(db: &PgPool, collective_id: Uuid) -> AppResult<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM memberships WHERE collective_id = $1")
            .bind(collective_id)
            .fetch_all(db)
            .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}

pub async fn collective_admin_ids(db: &PgPool, collective_id: Uuid) -> AppResult<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM memberships WHERE collective_id = $1 AND role = 'admin'",
    )
    .bind(collective_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}
