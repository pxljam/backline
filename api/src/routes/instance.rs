//! Instance administration (§3, §14). A single shared instance hosts every
//! collective; creating one is manual.

use crate::error::AppResult;
use crate::extract::Auth;
use crate::services::provisioning;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/collectives",
            get(list_collectives).post(create_collective),
        )
        .route("/health", get(technical_health))
}

#[derive(Serialize)]
struct CollectiveRow {
    id: Uuid,
    slug: String,
    name: String,
    members: i64,
    groups: i64,
    events: i64,
}

async fn list_collectives(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> AppResult<Json<Vec<CollectiveRow>>> {
    actor.require_instance_admin()?;
    let rows: Vec<(Uuid, String, String, i64, i64, i64)> = sqlx::query_as(
        "SELECT c.id, c.slug, c.name,
                (SELECT count(*) FROM memberships m WHERE m.collective_id = c.id),
                (SELECT count(*) FROM groups g WHERE g.collective_id = c.id),
                (SELECT count(*) FROM events e WHERE e.collective_id = c.id)
         FROM collectives c ORDER BY c.name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, slug, name, members, groups, events)| CollectiveRow {
                id,
                slug,
                name,
                members,
                groups,
                events,
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewCollective {
    slug: String,
    name: String,
    /// The collective's first admin. A collective without one is unusable.
    #[serde(default)]
    admin_user_id: Option<Uuid>,
}

async fn create_collective(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(body): Json<NewCollective>,
) -> AppResult<Json<serde_json::Value>> {
    actor.require_instance_admin()?;
    let id = provisioning::create_collective(&state.db, &body.slug, &body.name).await?;

    if let Some(user_id) = body.admin_user_id {
        sqlx::query(
            "INSERT INTO memberships (collective_id, user_id, role) VALUES ($1, $2, 'admin')
             ON CONFLICT (collective_id, user_id) DO UPDATE SET role = 'admin'",
        )
        .bind(id)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    }

    Ok(Json(json!({ "id": id })))
}

/// Technical health: what it takes to decide whether to step in.
async fn technical_health(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> AppResult<Json<serde_json::Value>> {
    actor.require_instance_admin()?;

    let (pending, failed): (i64, i64) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE status = 'pending'),
                count(*) FILTER (WHERE status = 'failed') FROM jobs",
    )
    .fetch_one(&state.db)
    .await?;

    let (machines,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM render_machines
         WHERE revoked_at IS NULL AND last_seen_at > now() - interval '10 minutes'",
    )
    .fetch_one(&state.db)
    .await?;

    let (queued_renders,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM render_jobs WHERE status = 'queued'")
            .fetch_one(&state.db)
            .await?;

    let (assets, bytes): (i64, Option<i64>) =
        sqlx::query_as("SELECT count(*), sum(bytes) FROM assets")
            .fetch_one(&state.db)
            .await?;

    Ok(Json(json!({
        "jobs": { "pending": pending, "failed": failed },
        "render": { "machines_online": machines, "queued": queued_renders },
        "storage": { "assets": assets, "bytes": bytes.unwrap_or(0) },
        "telegram": state.telegram.enabled(),
        "pdf": crate::services::pdf::available(),
    })))
}
