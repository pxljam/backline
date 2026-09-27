//! Instance administration (§3, §14). A single shared instance hosts every
//! collective; creating one is manual.

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{invitations, provisioning};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
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
        .route("/users", get(list_users).post(create_user))
        .route("/users/:user_id", axum::routing::patch(update_user))
        .route("/users/:user_id/invitation", post(regenerate_invitation))
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

// --- Users, instance-wide ----------------------------------------------------
//
// Every other creation path goes through a collective the acting admin belongs
// to (§3). That leaves two cases with nowhere to start: the very first
// collective, which needs an admin who does not exist yet, and a person who
// belongs to no collective at all. These three routes are that starting point,
// and they are the only place `is_instance_admin` is granted from the
// interface.

#[derive(Serialize)]
struct UserRow {
    id: Uuid,
    display_name: String,
    stage_name: Option<String>,
    email: Option<String>,
    is_instance_admin: bool,
    telegram_linked: bool,
    has_password: bool,
    /// A link was sent and not used yet — the person cannot sign in.
    pending_invitation: bool,
    collectives: Vec<String>,
}

async fn list_users(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> AppResult<Json<Vec<UserRow>>> {
    actor.require_instance_admin()?;
    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        Option<String>,
        bool,
        bool,
        bool,
        bool,
        Vec<String>,
    )> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.stage_name, u.email, u.is_instance_admin,
                    u.telegram_id IS NOT NULL, u.password_hash IS NOT NULL,
                    EXISTS (SELECT 1 FROM invitations i
                            WHERE i.user_id = u.id AND i.used_at IS NULL AND i.expires_at > now()),
                    COALESCE(ARRAY(SELECT c.name FROM memberships m
                                   JOIN collectives c ON c.id = m.collective_id
                                   WHERE m.user_id = u.id ORDER BY c.name), '{}')
             FROM users u ORDER BY u.is_instance_admin DESC, u.display_name",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    display_name,
                    stage_name,
                    email,
                    is_instance_admin,
                    telegram_linked,
                    has_password,
                    pending_invitation,
                    collectives,
                )| UserRow {
                    id,
                    display_name,
                    stage_name,
                    email,
                    is_instance_admin,
                    telegram_linked,
                    has_password,
                    pending_invitation,
                    collectives,
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewUser {
    display_name: String,
    #[serde(default)]
    stage_name: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    email: Option<String>,
    /// Granting it here is how a second instance admin comes to exist.
    #[serde(default)]
    is_instance_admin: bool,
}

async fn create_user(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(body): Json<NewUser>,
) -> AppResult<Json<serde_json::Value>> {
    actor.require_instance_admin()?;
    if body.display_name.trim().is_empty() {
        return Err(AppError::bad_request("le nom est obligatoire"));
    }

    let user_id = provisioning::upsert_user(
        &state.db,
        body.display_name.trim(),
        body.stage_name.as_deref(),
        body.phone.as_deref().unwrap_or(""),
        body.email.as_deref(),
    )
    .await?;

    if body.is_instance_admin {
        sqlx::query("UPDATE users SET is_instance_admin = TRUE WHERE id = $1")
            .bind(user_id)
            .execute(&state.db)
            .await?;
    }

    invitations::expire_pending(&state.db, user_id).await?;
    let code = invitations::issue(&state.db, user_id, Some(actor.user_id)).await?;

    Ok(Json(json!({
        "user_id": user_id,
        "invitation_code": code,
        "invitation_url": invitations::url(&state.config.public_base_url, &code),
    })))
}

#[derive(Deserialize)]
struct UpdateUser {
    is_instance_admin: bool,
}

async fn update_user(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(user_id): Path<Uuid>,
    Json(body): Json<UpdateUser>,
) -> AppResult<Json<serde_json::Value>> {
    actor.require_instance_admin()?;

    // Never remove the last one: §3 requires an instance admin who signs in
    // without Telegram, and nobody could grant the right back.
    if !body.is_instance_admin {
        let (admins,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM users WHERE is_instance_admin")
                .fetch_one(&state.db)
                .await?;
        let target: Option<(bool,)> =
            sqlx::query_as("SELECT is_instance_admin FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&state.db)
                .await?;
        let target = target.ok_or_else(|| AppError::not_found("utilisateur introuvable"))?;
        if admins <= 1 && target.0 {
            return Err(AppError::conflict(
                "c'est le dernier administrateur d'instance — en nommer un autre d'abord",
            ));
        }
    }

    sqlx::query("UPDATE users SET is_instance_admin = $2 WHERE id = $1")
        .bind(user_id)
        .bind(body.is_instance_admin)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// A new link for someone who lost theirs, or whose link expired. The previous
/// one stops working.
async fn regenerate_invitation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(user_id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    actor.require_instance_admin()?;

    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    exists.ok_or_else(|| AppError::not_found("utilisateur introuvable"))?;

    invitations::expire_pending(&state.db, user_id).await?;
    let code = invitations::issue(&state.db, user_id, Some(actor.user_id)).await?;

    Ok(Json(json!({
        "invitation_code": code,
        "invitation_url": invitations::url(&state.config.public_base_url, &code),
    })))
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
