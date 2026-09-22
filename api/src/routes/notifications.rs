//! Centre de notifications web — le doublon complet du bot (§13, §19).

use crate::error::AppResult;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list))
        .route("/read", post(mark_all_read))
        .route("/:id/read", post(mark_read))
        .route("/preferences", axum::routing::put(save_preferences))
}

#[derive(Serialize)]
struct NotificationRow {
    id: Uuid,
    collective_id: Option<Uuid>,
    kind: String,
    title: String,
    body: String,
    payload: Value,
    read_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(default)]
    unread_only: bool,
}

async fn list(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<NotificationRow>>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        Uuid,
        Option<Uuid>,
        String,
        String,
        String,
        Value,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, collective_id, kind, title, body, payload, read_at, created_at
             FROM notifications
             WHERE user_id = $1 AND ($2::bool IS NOT TRUE OR read_at IS NULL)
             ORDER BY created_at DESC LIMIT 200",
    )
    .bind(actor.user_id)
    .bind(q.unread_only)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, collective_id, kind, title, body, payload, read_at, created_at)| {
                    NotificationRow {
                        id,
                        collective_id,
                        kind,
                        title,
                        body,
                        payload,
                        read_at,
                        created_at,
                    }
                },
            )
            .collect(),
    ))
}

async fn mark_read(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE notifications SET read_at = now() WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(actor.user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn mark_all_read(State(state): State<AppState>, Auth(actor): Auth) -> AppResult<Json<Value>> {
    sqlx::query("UPDATE notifications SET read_at = now() WHERE user_id = $1 AND read_at IS NULL")
        .bind(actor.user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct Preferences {
    /// Silence nocturne : heures locales, 22 -> 8 par exemple.
    #[serde(default)]
    quiet_from: Option<i16>,
    #[serde(default)]
    quiet_to: Option<i16>,
    /// Opt-out par type de notification (§13).
    #[serde(default)]
    opt_out: Vec<String>,
}

async fn save_preferences(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(body): Json<Preferences>,
) -> AppResult<Json<Value>> {
    sqlx::query(
        "UPDATE users SET notif_quiet_from = $2, notif_quiet_to = $3, notif_opt_out = $4,
                          updated_at = now() WHERE id = $1",
    )
    .bind(actor.user_id)
    .bind(body.quiet_from)
    .bind(body.quiet_to)
    .bind(&body.opt_out)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}
