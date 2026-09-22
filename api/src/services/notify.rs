//! Centre de notifications web + delivrance Telegram (§13, §14).
//!
//! Une requete HTTP ne parle jamais a Telegram : elle **depose** une
//! notification, qu'un job delivre. Un reseau lent ou une API Telegram en panne
//! ne peut donc pas faire echouer une action metier.

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

/// Respecte le silence nocturne et les opt-out par type (§13).
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
                // plage a cheval sur minuit : 22h -> 8h
                hour >= from || hour < to
            }
        }
        _ => false,
    }
}

/// Delivre les notifications en attente vers Telegram. Les alertes critiques
/// (« on est en ligne », alerte admin) traversent le silence nocturne.
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
            // Une notification qui appelle une action part avec son bouton :
            // deux appuis, pas un aller-retour par le web (§13).
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
                tracing::warn!(error = %e, "delivrance telegram echouee");
                continue;
            }
            sent += 1;
        }
        // Une notification silencieuse reste visible dans le centre web : on la
        // marque delivree pour ne pas la rejouer indefiniment.
        sqlx::query("UPDATE notifications SET delivered_telegram = TRUE WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
    }
    Ok(sent)
}

/// Tous les membres d'un collectif.
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
