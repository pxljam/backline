//! Dashboard: **what is blocking** (§14).

use crate::error::AppResult;
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new().route("/dashboard", get(dashboard))
}

#[derive(Serialize)]
struct PendingPoll {
    opportunity_id: Uuid,
    title: String,
    dates: i64,
    /// `true` if *I* have not answered yet — that is the expected action.
    mine_missing: bool,
}

#[derive(Serialize)]
struct VacantSlot {
    event_id: Uuid,
    event_title: String,
    starts_at: DateTime<Utc>,
    label: String,
    vacant: i64,
}

#[derive(Serialize)]
struct LateTask {
    task_id: Uuid,
    event_title: String,
    label: String,
    scheduled_at: DateTime<Utc>,
    status: String,
    assignee_id: Option<Uuid>,
}

async fn dashboard(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    let me = scope.user_id();

    let polls: Vec<(Uuid, String, i64, i64)> = sqlx::query_as(
        "SELECT o.id, o.title,
                (SELECT count(*) FROM candidate_dates d WHERE d.opportunity_id = o.id),
                (SELECT count(*) FROM availabilities a
                   JOIN candidate_dates d ON d.id = a.candidate_date_id
                   WHERE d.opportunity_id = o.id AND a.user_id = $2)
         FROM opportunities o
         JOIN availability_polls p ON p.opportunity_id = o.id
         WHERE o.collective_id = $1 AND p.closed_at IS NULL
         ORDER BY o.created_at DESC",
    )
    .bind(cid)
    .bind(me)
    .fetch_all(&state.db)
    .await?;

    let pending_polls: Vec<PendingPoll> = polls
        .into_iter()
        .map(|(opportunity_id, title, dates, mine)| PendingPoll {
            opportunity_id,
            title,
            dates,
            mine_missing: mine < dates,
        })
        .collect();

    let vacant: Vec<(Uuid, String, DateTime<Utc>, String, i64)> = sqlx::query_as(
        "SELECT e.id, e.title, e.starts_at, s.label, s.quantity - count(a.id)
         FROM logistics_slots s
         JOIN events e ON e.id = s.event_id
         LEFT JOIN logistics_assignments a ON a.slot_id = s.id
         WHERE e.collective_id = $1 AND e.status = 'confirmed' AND e.starts_at > now()
         GROUP BY e.id, e.title, e.starts_at, s.id, s.label, s.quantity
         HAVING count(a.id) < s.quantity
         ORDER BY e.starts_at",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let late: Vec<(Uuid, String, String, DateTime<Utc>, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT p.id, e.title, p.label, p.scheduled_at, p.status, p.assignee_id
         FROM publication_tasks p JOIN events e ON e.id = p.event_id
         WHERE e.collective_id = $1 AND p.status <> 'published' AND p.scheduled_at < now()
         ORDER BY p.scheduled_at",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    // The next 30 days.
    let upcoming: Vec<(Uuid, String, DateTime<Utc>, String, Option<String>)> = sqlx::query_as(
        "SELECT e.id, e.title, e.starts_at, t.key, v.name
         FROM events e
         JOIN event_types t ON t.id = e.event_type_id
         LEFT JOIN venues v ON v.id = e.venue_id
         WHERE e.collective_id = $1 AND e.status = 'confirmed'
           AND e.starts_at BETWEEN now() AND now() + interval '30 days'
         ORDER BY e.starts_at",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    // Missing tech riders: reported, never blocking (§12).
    let missing_riders: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT e.id, e.title, g.name
         FROM event_tech_riders etr
         JOIN events e ON e.id = etr.event_id
         JOIN groups g ON g.id = etr.group_id
         WHERE e.collective_id = $1 AND etr.tech_rider_id IS NULL AND e.starts_at > now()
         ORDER BY e.starts_at",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let (machines_online,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM render_machines
         WHERE revoked_at IS NULL AND last_seen_at > now() - interval '10 minutes'",
    )
    .fetch_one(&state.db)
    .await?;

    let (unread,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL")
            .bind(me)
            .fetch_one(&state.db)
            .await?;

    Ok(Json(json!({
        "pending_polls": pending_polls,
        "vacant_slots": vacant.into_iter().map(|(event_id, event_title, starts_at, label, vacant)| {
            VacantSlot { event_id, event_title, starts_at, label, vacant }
        }).collect::<Vec<_>>(),
        "late_tasks": late.into_iter().map(|(task_id, event_title, label, scheduled_at, status, assignee_id)| {
            LateTask { task_id, event_title, label, scheduled_at, status, assignee_id }
        }).collect::<Vec<_>>(),
        "upcoming": upcoming.into_iter().map(|(id, title, starts_at, type_key, venue)| json!({
            "id": id, "title": title, "starts_at": starts_at, "type_key": type_key, "venue": venue
        })).collect::<Vec<_>>(),
        "missing_tech_riders": missing_riders.into_iter().map(|(event_id, event_title, group_name)| json!({
            "event_id": event_id, "event_title": event_title, "group_name": group_name
        })).collect::<Vec<_>>(),
        "render_machines_online": machines_online,
        "unread_notifications": unread,
        "is_admin": scope.is_admin(),
    })))
}
