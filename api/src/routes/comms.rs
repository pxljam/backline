//! Comms plan and assisted-mode publishing (§11).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{jobs, notify, visuals};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/events/:event_id/comms", get(event_plan))
        .route("/events/:event_id/comms/generate", post(generate_visuals))
        .route("/publication-tasks/:task_id", patch(update_task))
        .route("/publication-tasks/:task_id/assign", post(assign_task))
        .route(
            "/publication-tasks/:task_id/published",
            post(mark_published),
        )
        .route("/publication-tasks", get(my_tasks))
}

#[derive(Serialize)]
pub struct TaskRow {
    pub id: Uuid,
    pub event_id: Uuid,
    pub event_title: String,
    pub milestone_key: String,
    pub label: String,
    pub status: String,
    pub scheduled_at: DateTime<Utc>,
    pub social_account: Option<AccountBrief>,
    pub assignee_id: Option<Uuid>,
    pub backup_assignee_id: Option<Uuid>,
    pub caption: String,
    pub hashtags: String,
    pub formats: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub published_url: Option<String>,
    pub reminder_count: i32,
    pub visuals: Vec<VisualRow>,
}

#[derive(Serialize)]
pub struct AccountBrief {
    pub id: Uuid,
    pub platform: String,
    pub handle: String,
}

#[derive(Serialize)]
pub struct VisualRow {
    pub format_id: Uuid,
    pub format_key: String,
    pub status: String,
    pub asset_id: Option<Uuid>,
    pub error: Option<String>,
}

async fn load_tasks(
    state: &AppState,
    cid: Uuid,
    where_sql: &str,
    bind: Uuid,
) -> AppResult<Vec<TaskRow>> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        String,
        String,
        String,
        DateTime<Utc>,
        Option<Uuid>,
        Option<Uuid>,
        Option<Uuid>,
        String,
        String,
        Value,
        Option<DateTime<Utc>>,
        Option<String>,
        i32,
    )> = sqlx::query_as(&format!(
        "SELECT p.id, p.event_id, e.title, p.milestone_key, p.label, p.status, p.scheduled_at,
                    p.social_account_id, p.assignee_id, p.backup_assignee_id, p.caption, p.hashtags,
                    p.formats, p.published_at, p.published_url, p.reminder_count
             FROM publication_tasks p JOIN events e ON e.id = p.event_id
             WHERE e.collective_id = $1 AND {where_sql}
             ORDER BY p.scheduled_at"
    ))
    .bind(cid)
    .bind(bind)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for (
        id,
        event_id,
        event_title,
        milestone_key,
        label,
        status,
        scheduled_at,
        account_id,
        assignee_id,
        backup_assignee_id,
        caption,
        hashtags,
        formats,
        published_at,
        published_url,
        reminder_count,
    ) in rows
    {
        let social_account = match account_id {
            Some(aid) => {
                let a: (Uuid, String, String) = sqlx::query_as(
                    "SELECT id, platform, handle FROM social_accounts WHERE id = $1",
                )
                .bind(aid)
                .fetch_one(&state.db)
                .await?;
                Some(AccountBrief {
                    id: a.0,
                    platform: a.1,
                    handle: a.2,
                })
            }
            None => None,
        };

        let visuals: Vec<(Uuid, String, String, Option<Uuid>, Option<String>)> = sqlx::query_as(
            "SELECT pa.format_id, f.key, pa.status, pa.asset_id, pa.error
             FROM publication_assets pa JOIN formats f ON f.id = pa.format_id
             WHERE pa.publication_task_id = $1",
        )
        .bind(id)
        .fetch_all(&state.db)
        .await?;

        out.push(TaskRow {
            id,
            event_id,
            event_title,
            milestone_key,
            label,
            status,
            scheduled_at,
            social_account,
            assignee_id,
            backup_assignee_id,
            caption,
            hashtags,
            formats,
            published_at,
            published_url,
            reminder_count,
            visuals: visuals
                .into_iter()
                .map(
                    |(format_id, format_key, status, asset_id, error)| VisualRow {
                        format_id,
                        format_key,
                        status,
                        asset_id,
                        error,
                    },
                )
                .collect(),
        });
    }
    Ok(out)
}

async fn event_plan(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<TaskRow>>> {
    let _scope = state.scope(actor, cid).await?;
    crate::routes::events::check_event(&state, cid, eid).await?;
    Ok(Json(load_tasks(&state, cid, "p.event_id = $2", eid).await?))
}

/// The tasks I own — the list the bot reminds me about.
async fn my_tasks(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<TaskRow>>> {
    let scope = state.scope(actor, cid).await?;
    Ok(Json(
        load_tasks(
            &state,
            cid,
            "(p.assignee_id = $2 OR p.backup_assignee_id = $2) AND p.status <> 'published'",
            scope.user_id(),
        )
        .await?,
    ))
}

/// "Generate the visuals": every format in the plan, in one action (§9.4).
/// The work goes into the queue — the admin does not wait on a spinner.
async fn generate_visuals(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    crate::routes::events::check_event(&state, cid, eid).await?;

    jobs::enqueue(
        &state.db,
        "generate_visuals",
        Utc::now(),
        json!({ "event_id": eid }),
        // No deduplication key: re-running generation is a deliberate action,
        // after a template fix for instance.
        None,
    )
    .await?;

    Ok(Json(json!({ "queued": true })))
}

#[derive(Deserialize)]
struct UpdateTask {
    #[serde(default)]
    caption: Option<String>,
    #[serde(default)]
    hashtags: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    social_account_id: Option<Uuid>,
    #[serde(default)]
    scheduled_at: Option<DateTime<Utc>>,
}

async fn update_task(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateTask>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_task(&state, cid, tid).await?;

    if let Some(s) = &body.status {
        // `published` is only reached by an explicit human action (§18).
        if s == "published" {
            return Err(AppError::bad_request(
                "utiliser « ✅ publie » pour confirmer une publication",
            ));
        }
        if !matches!(s.as_str(), "draft" | "ready" | "assigned" | "missed") {
            return Err(AppError::bad_request("statut inconnu"));
        }
    }

    sqlx::query(
        "UPDATE publication_tasks SET
            caption = COALESCE($2, caption),
            hashtags = COALESCE($3, hashtags),
            status = COALESCE($4, status),
            social_account_id = COALESCE($5, social_account_id),
            scheduled_at = COALESCE($6, scheduled_at),
            updated_at = now()
         WHERE id = $1",
    )
    .bind(tid)
    .bind(&body.caption)
    .bind(&body.hashtags)
    .bind(&body.status)
    .bind(body.social_account_id)
    .bind(body.scheduled_at)
    .execute(&state.db)
    .await?;

    if let Some(at) = body.scheduled_at {
        jobs::enqueue(
            &state.db,
            "publication_due",
            at,
            json!({ "task_id": tid }),
            Some(format!("task:{tid}:due:{}", at.timestamp())),
        )
        .await?;
    }

    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AssignTask {
    #[serde(default)]
    assignee_id: Option<Uuid>,
    #[serde(default)]
    backup_assignee_id: Option<Uuid>,
}

/// A task can only be assigned to a member **with access to the account** in
/// question (§11.2, §18). This is the rule that avoids the day-of back and
/// forth.
async fn assign_task(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
    Json(body): Json<AssignTask>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_task(&state, cid, tid).await?;

    // A member can nominate themselves; assigning someone else is an admin
    // action.
    let assigning_self =
        body.assignee_id == Some(scope.user_id()) && body.backup_assignee_id.is_none();
    if !assigning_self {
        scope.require_admin()?;
    }

    let account: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT social_account_id FROM publication_tasks WHERE id = $1")
            .bind(tid)
            .fetch_optional(&state.db)
            .await?;
    let account_id = account
        .and_then(|(a,)| a)
        .ok_or_else(|| AppError::bad_request("rattacher d'abord cette tache a un compte social"))?;

    for candidate in [body.assignee_id, body.backup_assignee_id]
        .into_iter()
        .flatten()
    {
        let has_access: Option<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM social_account_access
             WHERE social_account_id = $1 AND user_id = $2",
        )
        .bind(account_id)
        .bind(candidate)
        .fetch_optional(&state.db)
        .await?;
        if has_access.is_none() {
            return Err(AppError::forbidden(
                "cette personne n'a pas acces au compte vise — lui donner l'acces d'abord",
            ));
        }
    }

    sqlx::query(
        "UPDATE publication_tasks SET
            assignee_id = $2, backup_assignee_id = $3,
            status = CASE WHEN $2::uuid IS NOT NULL AND status IN ('draft','ready')
                          THEN 'assigned' ELSE status END,
            updated_at = now()
         WHERE id = $1",
    )
    .bind(tid)
    .bind(body.assignee_id)
    .bind(body.backup_assignee_id)
    .execute(&state.db)
    .await?;

    if let Some(user_id) = body.assignee_id {
        let (label, when, collective_id): (String, DateTime<Utc>, Uuid) = sqlx::query_as(
            "SELECT p.label, p.scheduled_at, e.collective_id FROM publication_tasks p
             JOIN events e ON e.id = p.event_id WHERE p.id = $1",
        )
        .bind(tid)
        .fetch_one(&state.db)
        .await?;
        notify::push(
            &state.db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                kind: "publication_assigned",
                title: format!("Tu publies : {label}"),
                body: format!(
                    "Prevu le {}",
                    when.with_timezone(&chrono_tz::Europe::Paris)
                        .format("%d/%m a %H:%M")
                ),
                payload: json!({ "task_id": tid }),
            },
        )
        .await?;
    }

    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct MarkPublished {
    #[serde(default)]
    published_url: Option<String>,
}

/// "✅ publie" — **the only route** to the published status. No API does it in
/// a human's place (§11.2).
async fn mark_published(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
    Json(body): Json<MarkPublished>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_task(&state, cid, tid).await?;

    let row: Option<(Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT assignee_id, backup_assignee_id FROM publication_tasks WHERE id = $1",
    )
    .bind(tid)
    .fetch_optional(&state.db)
    .await?;
    let (assignee, backup) = row.ok_or_else(|| AppError::not_found("tache introuvable"))?;

    // The owner, their stand-in, or an admin who published in their place.
    let me = scope.user_id();
    if assignee != Some(me) && backup != Some(me) && !scope.is_admin() {
        return Err(AppError::forbidden("cette tache ne t'est pas assignee"));
    }

    sqlx::query(
        "UPDATE publication_tasks SET status = 'published', published_at = now(),
                                      published_url = $2, updated_at = now()
         WHERE id = $1",
    )
    .bind(tid)
    .bind(&body.published_url)
    .execute(&state.db)
    .await?;

    // The scheduled reminders no longer have a reason to exist.
    jobs::cancel_by_prefix(&state.db, &format!("task:{tid}:")).await?;

    Ok(Json(json!({ "ok": true })))
}

async fn check_task(state: &AppState, cid: Uuid, tid: Uuid) -> AppResult<()> {
    let ok: Option<(Uuid,)> = sqlx::query_as(
        "SELECT p.id FROM publication_tasks p JOIN events e ON e.id = p.event_id
         WHERE p.id = $1 AND e.collective_id = $2",
    )
    .bind(tid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    ok.map(|_| ())
        .ok_or_else(|| AppError::not_found("tache introuvable"))
}

/// Exposed for the scheduler: generating the visuals is a job.
pub async fn run_generate_visuals(state: &AppState, event_id: Uuid) -> AppResult<()> {
    let (ok, failed) = visuals::generate_for_event(state, event_id).await?;
    tracing::info!(event = %event_id, ok, failed, "visuals generated");
    Ok(())
}
