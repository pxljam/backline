//! Evenements, line-up, logistique, streams, feuille de route (§5, §6).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{comms, events as events_svc, run_sheet};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/:eid", get(show).patch(update))
        .route("/:eid/participations", post(add_participation))
        .route(
            "/:eid/participations/:pid",
            patch(update_participation).delete(remove_participation),
        )
        .route("/:eid/logistics", post(add_slot))
        .route("/:eid/logistics/:sid", delete(remove_slot))
        .route(
            "/:eid/logistics/:sid/take",
            post(take_slot).delete(release_slot),
        )
        .route("/:eid/presence", post(set_presence))
        .route("/:eid/stream", patch(update_stream))
        .route("/:eid/run-sheet", get(get_run_sheet))
        .route("/:eid/labels", get(logistics_label_suggestions))
}

#[derive(Serialize)]
pub struct EventRow {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub type_key: String,
    pub type_label: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub doors_at: Option<DateTime<Utc>>,
    pub soundcheck_at: Option<DateTime<Utc>>,
    pub set_times_state: String,
    pub set_times_public: bool,
    pub venue: Option<VenueBrief>,
    pub host_group_id: Option<Uuid>,
    pub participations: Vec<Participation>,
    pub logistics: Vec<Slot>,
    pub stream: Option<Stream>,
    pub residency: Option<Residency>,
    pub tech_riders: Vec<RiderState>,
    pub comms_summary: CommsSummary,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct VenueBrief {
    pub id: Uuid,
    pub name: String,
    pub city: Option<String>,
    pub address: Option<String>,
}

#[derive(Serialize)]
pub struct Participation {
    pub id: Uuid,
    pub group_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub label: String,
    pub status: String,
    pub stage_role: Option<String>,
    pub slot_start: Option<NaiveTime>,
    pub slot_end: Option<NaiveTime>,
    pub acknowledged: bool,
}

#[derive(Serialize)]
pub struct Slot {
    pub id: Uuid,
    pub label: String,
    pub quantity: i32,
    pub notes: Option<String>,
    pub assignees: Vec<Assignee>,
    pub vacant: i32,
}

#[derive(Serialize)]
pub struct Assignee {
    pub user_id: Uuid,
    pub name: String,
}

#[derive(Serialize)]
pub struct Stream {
    pub capture_location: Option<String>,
    pub planned_duration_min: Option<i32>,
    pub replay_url: Option<String>,
    pub live_alert_sent_at: Option<DateTime<Utc>>,
    pub platforms: Vec<StreamPlatform>,
}

#[derive(Serialize, Deserialize)]
pub struct StreamPlatform {
    pub platform: String,
    #[serde(default)]
    pub url: Option<String>,
}

#[derive(Serialize)]
pub struct Residency {
    pub presences: Vec<Presence>,
}

#[derive(Serialize)]
pub struct Presence {
    pub user_id: Uuid,
    pub name: String,
    pub answer: String,
    /// Jours precis : facultatifs, jamais reclames (§6).
    pub days: Vec<NaiveDate>,
}

#[derive(Serialize)]
pub struct RiderState {
    pub group_id: Uuid,
    pub group_name: String,
    pub tech_rider_id: Option<Uuid>,
    pub version: Option<i32>,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CommsSummary {
    pub total: i64,
    pub published: i64,
    pub late: i64,
    pub unassigned: i64,
}

#[derive(Deserialize)]
pub struct ListQuery {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub from: Option<DateTime<Utc>>,
    #[serde(default)]
    pub to: Option<DateTime<Utc>>,
    #[serde(default)]
    pub mine: Option<bool>,
}

async fn list(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Query(q): Query<ListQuery>,
) -> AppResult<Json<Vec<EventRow>>> {
    let scope = state.scope(actor, cid).await?;
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT e.id FROM events e
         WHERE e.collective_id = $1
           AND ($2::text IS NULL OR e.status = $2)
           AND ($3::timestamptz IS NULL OR e.starts_at >= $3)
           AND ($4::timestamptz IS NULL OR e.starts_at <= $4)
           AND ($5::bool IS NOT TRUE OR EXISTS (
                 SELECT 1 FROM participations p
                 LEFT JOIN group_members gm ON gm.group_id = p.group_id
                 WHERE p.event_id = e.id AND (p.user_id = $6 OR gm.user_id = $6)
                 UNION
                 SELECT 1 FROM logistics_assignments a
                 JOIN logistics_slots s ON s.id = a.slot_id
                 WHERE s.event_id = e.id AND a.user_id = $6))
         ORDER BY e.starts_at",
    )
    .bind(cid)
    .bind(&q.status)
    .bind(q.from)
    .bind(q.to)
    .bind(q.mine)
    .bind(scope.user_id())
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for (id,) in rows {
        out.push(load(&state, cid, id).await?);
    }
    Ok(Json(out))
}

pub async fn load(state: &AppState, cid: Uuid, eid: Uuid) -> AppResult<EventRow> {
    #[allow(clippy::type_complexity)]
    let row: Option<(
        Uuid,
        String,
        String,
        String,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        String,
        bool,
        Option<Uuid>,
        Option<Uuid>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT e.id, e.title, e.status, t.key, t.label, e.starts_at, e.ends_at, e.doors_at,
                e.soundcheck_at, e.set_times_state, e.set_times_public, e.venue_id,
                e.host_group_id, e.notes
         FROM events e JOIN event_types t ON t.id = e.event_type_id
         WHERE e.id = $1 AND e.collective_id = $2",
    )
    .bind(eid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;

    let (
        id,
        title,
        status,
        type_key,
        type_label,
        starts_at,
        ends_at,
        doors_at,
        soundcheck_at,
        set_times_state,
        set_times_public,
        venue_id,
        host_group_id,
        notes,
    ) = row.ok_or_else(|| AppError::not_found("evenement introuvable"))?;

    let venue = match venue_id {
        Some(vid) => {
            let v: (Uuid, String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT id, name, city, address FROM venues WHERE id = $1")
                    .bind(vid)
                    .fetch_one(&state.db)
                    .await?;
            Some(VenueBrief {
                id: v.0,
                name: v.1,
                city: v.2,
                address: v.3,
            })
        }
        None => None,
    };

    #[allow(clippy::type_complexity)]
    let parts: Vec<(
        Uuid,
        Option<Uuid>,
        Option<String>,
        Option<Uuid>,
        Option<String>,
        String,
        Option<String>,
        Option<NaiveTime>,
        Option<NaiveTime>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT p.id, p.group_id, g.name, p.user_id, COALESCE(u.stage_name, u.display_name),
                    p.status, p.stage_role, p.slot_start, p.slot_end, p.acknowledged_at
             FROM participations p
             LEFT JOIN groups g ON g.id = p.group_id
             LEFT JOIN users u ON u.id = p.user_id
             WHERE p.event_id = $1 ORDER BY p.position",
    )
    .bind(eid)
    .fetch_all(&state.db)
    .await?;

    let participations = parts
        .into_iter()
        .map(
            |(id, group_id, gname, user_id, uname, status, stage_role, s, e, ack)| Participation {
                id,
                group_id,
                user_id,
                label: gname.or(uname).unwrap_or_else(|| "?".into()),
                status,
                stage_role,
                // Les creneaux ne sortent que s'ils sont publiables (§6). Ici c'est
                // la vue interne : on les montre, avec leur etat.
                slot_start: s,
                slot_end: e,
                acknowledged: ack.is_some(),
            },
        )
        .collect();

    let slot_rows: Vec<(Uuid, String, i32, Option<String>)> = sqlx::query_as(
        "SELECT id, label, quantity, notes FROM logistics_slots WHERE event_id = $1 ORDER BY position",
    )
    .bind(eid)
    .fetch_all(&state.db)
    .await?;

    let mut logistics = Vec::new();
    for (sid, label, quantity, notes) in slot_rows {
        let a: Vec<(Uuid, String)> = sqlx::query_as(
            "SELECT u.id, COALESCE(u.stage_name, u.display_name)
             FROM logistics_assignments la JOIN users u ON u.id = la.user_id
             WHERE la.slot_id = $1",
        )
        .bind(sid)
        .fetch_all(&state.db)
        .await?;
        let assignees: Vec<Assignee> = a
            .into_iter()
            .map(|(user_id, name)| Assignee { user_id, name })
            .collect();
        let vacant = (quantity - assignees.len() as i32).max(0);
        logistics.push(Slot {
            id: sid,
            label,
            quantity,
            notes,
            assignees,
            vacant,
        });
    }

    let stream = if type_key == "stream" {
        let s: Option<(
            Option<String>,
            Option<i32>,
            Option<String>,
            Option<DateTime<Utc>>,
        )> = sqlx::query_as(
            "SELECT capture_location, planned_duration_min, replay_url, live_alert_sent_at
                 FROM event_streams WHERE event_id = $1",
        )
        .bind(eid)
        .fetch_optional(&state.db)
        .await?;
        let platforms: Vec<(String, Option<String>)> =
            sqlx::query_as("SELECT platform, url FROM event_stream_platforms WHERE event_id = $1")
                .bind(eid)
                .fetch_all(&state.db)
                .await?;
        let (capture_location, planned_duration_min, replay_url, live_alert_sent_at) =
            s.unwrap_or((None, None, None, None));
        Some(Stream {
            capture_location,
            planned_duration_min,
            replay_url,
            live_alert_sent_at,
            platforms: platforms
                .into_iter()
                .map(|(platform, url)| StreamPlatform { platform, url })
                .collect(),
        })
    } else {
        None
    };

    let residency = if type_key == "residency" {
        let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
            "SELECT rp.id, rp.user_id, COALESCE(u.stage_name, u.display_name), rp.answer
             FROM residency_presences rp JOIN users u ON u.id = rp.user_id
             WHERE rp.event_id = $1",
        )
        .bind(eid)
        .fetch_all(&state.db)
        .await?;
        let mut presences = Vec::new();
        for (pid, user_id, name, answer) in rows {
            let days: Vec<(NaiveDate,)> = sqlx::query_as(
                "SELECT day FROM residency_presence_days WHERE presence_id = $1 ORDER BY day",
            )
            .bind(pid)
            .fetch_all(&state.db)
            .await?;
            presences.push(Presence {
                user_id,
                name,
                answer,
                days: days.into_iter().map(|(d,)| d).collect(),
            });
        }
        Some(Residency { presences })
    } else {
        None
    };

    let riders: Vec<(
        Uuid,
        String,
        Option<Uuid>,
        Option<i32>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT etr.group_id, g.name, etr.tech_rider_id, r.version, etr.sent_at
             FROM event_tech_riders etr
             JOIN groups g ON g.id = etr.group_id
             LEFT JOIN tech_riders r ON r.id = etr.tech_rider_id
             WHERE etr.event_id = $1",
    )
    .bind(eid)
    .fetch_all(&state.db)
    .await?;

    let (total, published, late, unassigned): (i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*),
                count(*) FILTER (WHERE status = 'published'),
                count(*) FILTER (WHERE status <> 'published' AND scheduled_at < now()),
                count(*) FILTER (WHERE assignee_id IS NULL AND status <> 'published')
         FROM publication_tasks WHERE event_id = $1",
    )
    .bind(eid)
    .fetch_one(&state.db)
    .await?;

    Ok(EventRow {
        id,
        title,
        status,
        type_key,
        type_label,
        starts_at,
        ends_at,
        doors_at,
        soundcheck_at,
        set_times_state,
        set_times_public,
        venue,
        host_group_id,
        participations,
        logistics,
        stream,
        residency,
        tech_riders: riders
            .into_iter()
            .map(
                |(group_id, group_name, tech_rider_id, version, sent_at)| RiderState {
                    group_id,
                    group_name,
                    tech_rider_id,
                    version,
                    sent_at,
                },
            )
            .collect(),
        comms_summary: CommsSummary {
            total,
            published,
            late,
            unassigned,
        },
        notes,
    })
}

async fn show(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<EventRow>> {
    let _scope = state.scope(actor, cid).await?;
    Ok(Json(load(&state, cid, eid).await?))
}

#[derive(Deserialize)]
struct NewEvent {
    event_type_key: String,
    title: String,
    starts_at: DateTime<Utc>,
    #[serde(default)]
    ends_at: Option<DateTime<Utc>>,
    #[serde(default)]
    venue_id: Option<Uuid>,
    #[serde(default)]
    host_group_id: Option<Uuid>,
    #[serde(default)]
    notes: Option<String>,
    /// Streams : plateformes de diffusion, multi-diffusion possible (§6).
    #[serde(default)]
    platforms: Vec<StreamPlatform>,
    #[serde(default)]
    capture_location: Option<String>,
    #[serde(default)]
    planned_duration_min: Option<i32>,
}

/// Creation directe. Les **streams** passent par ici : pas de lieu a negocier,
/// donc pas d'opportunite (§5.1).
async fn create(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewEvent>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let t: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, is_range FROM event_types WHERE collective_id = $1 AND key = $2",
    )
    .bind(cid)
    .bind(&body.event_type_key)
    .fetch_optional(&state.db)
    .await?;
    let (event_type_id, is_range) =
        t.ok_or_else(|| AppError::not_found("type d'evenement inconnu"))?;

    // Une residence est un evenement unique portant une plage : la fin est
    // obligatoire, sinon ce n'est pas une residence (§6).
    if is_range && body.ends_at.is_none() {
        return Err(AppError::bad_request(
            "une residence porte une date de debut et une date de fin",
        ));
    }
    if let Some(gid) = body.host_group_id {
        scope.check_group(&state.db, gid).await?;
    }

    let (event_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO events
            (collective_id, event_type_id, venue_id, host_group_id, title, status, starts_at, ends_at, notes)
         VALUES ($1, $2, $3, $4, $5, 'confirmed', $6, $7, $8) RETURNING id",
    )
    .bind(cid)
    .bind(event_type_id)
    .bind(body.venue_id)
    .bind(body.host_group_id)
    .bind(&body.title)
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(&body.notes)
    .fetch_one(&state.db)
    .await?;

    if body.event_type_key == "stream" {
        sqlx::query(
            "INSERT INTO event_streams (event_id, capture_location, planned_duration_min)
             VALUES ($1, $2, $3)",
        )
        .bind(event_id)
        .bind(&body.capture_location)
        .bind(body.planned_duration_min)
        .execute(&state.db)
        .await?;
        for p in &body.platforms {
            sqlx::query(
                "INSERT INTO event_stream_platforms (event_id, platform, url) VALUES ($1, $2, $3)",
            )
            .bind(event_id)
            .bind(&p.platform)
            .bind(&p.url)
            .execute(&state.db)
            .await?;
        }
    }

    for (i, (label, qty)) in comms::default_logistics(&body.event_type_key)
        .iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO logistics_slots (event_id, label, quantity, position) VALUES ($1, $2, $3, $4)",
        )
        .bind(event_id)
        .bind(label)
        .bind(qty)
        .bind(i as i32)
        .execute(&state.db)
        .await?;
    }

    comms::instantiate_plan(&state.db, event_id).await?;
    events_svc::schedule_event_jobs(&state.db, event_id).await?;

    Ok(Json(json!({ "id": event_id })))
}

#[derive(Deserialize)]
struct UpdateEvent {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    starts_at: Option<DateTime<Utc>>,
    #[serde(default)]
    ends_at: Option<DateTime<Utc>>,
    #[serde(default)]
    doors_at: Option<DateTime<Utc>>,
    #[serde(default)]
    soundcheck_at: Option<DateTime<Utc>>,
    #[serde(default)]
    set_times_state: Option<String>,
    #[serde(default)]
    set_times_public: Option<bool>,
    #[serde(default)]
    notes: Option<String>,
}

async fn update(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateEvent>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    let before: Option<(DateTime<Utc>,)> =
        sqlx::query_as("SELECT starts_at FROM events WHERE id = $1 AND collective_id = $2")
            .bind(eid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    let (old_start,) = before.ok_or_else(|| AppError::not_found("evenement introuvable"))?;

    if let Some(s) = &body.set_times_state {
        if !matches!(s.as_str(), "undefined" | "to_confirm" | "defined") {
            return Err(AppError::bad_request("etat de creneaux inconnu"));
        }
    }

    sqlx::query(
        "UPDATE events SET
            title = COALESCE($3, title),
            status = COALESCE($4, status),
            starts_at = COALESCE($5, starts_at),
            ends_at = COALESCE($6, ends_at),
            doors_at = COALESCE($7, doors_at),
            soundcheck_at = COALESCE($8, soundcheck_at),
            set_times_state = COALESCE($9, set_times_state),
            set_times_public = COALESCE($10, set_times_public),
            notes = COALESCE($11, notes),
            updated_at = now()
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(eid)
    .bind(cid)
    .bind(&body.title)
    .bind(&body.status)
    .bind(body.starts_at)
    .bind(body.ends_at)
    .bind(body.doors_at)
    .bind(body.soundcheck_at)
    .bind(&body.set_times_state)
    .bind(body.set_times_public)
    .bind(&body.notes)
    .execute(&state.db)
    .await?;

    // La date a bouge : tout ce qui etait programme dessus est faux. On annule
    // et on reprogramme, plutot que d'envoyer un rappel J-7 le lendemain.
    if body.starts_at.is_some_and(|s| s != old_start) {
        crate::services::jobs::cancel_by_prefix(&state.db, &format!("event:{eid}:")).await?;
        sqlx::query("DELETE FROM publication_tasks WHERE event_id = $1 AND status = 'draft'")
            .bind(eid)
            .execute(&state.db)
            .await?;
        comms::instantiate_plan(&state.db, eid).await?;
        events_svc::schedule_event_jobs(&state.db, eid).await?;
    }

    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct NewParticipation {
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    user_id: Option<Uuid>,
    #[serde(default)]
    stage_role: Option<String>,
    #[serde(default)]
    slot_start: Option<NaiveTime>,
    #[serde(default)]
    slot_end: Option<NaiveTime>,
}

async fn add_participation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewParticipation>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_event(&state, cid, eid).await?;
    if body.group_id.is_none() && body.user_id.is_none() {
        return Err(AppError::bad_request("viser un groupe ou une personne"));
    }
    // Un groupe invite peut venir d'un autre collectif (§4) : on ne verifie
    // donc pas son rattachement, seulement son existence.
    if let Some(gid) = body.group_id {
        let ok: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM groups WHERE id = $1")
            .bind(gid)
            .fetch_optional(&state.db)
            .await?;
        ok.ok_or_else(|| AppError::not_found("groupe introuvable"))?;
    }

    let (pos,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(max(position) + 1, 0) FROM participations WHERE event_id = $1",
    )
    .bind(eid)
    .fetch_one(&state.db)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO participations (event_id, group_id, user_id, stage_role, slot_start, slot_end, position)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(eid)
    .bind(body.group_id)
    .bind(body.user_id)
    .bind(&body.stage_role)
    .bind(body.slot_start)
    .bind(body.slot_end)
    .bind(pos)
    .fetch_one(&state.db)
    .await?;

    events_svc::attach_tech_riders(&state.db, eid).await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct UpdateParticipation {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    stage_role: Option<String>,
    #[serde(default)]
    slot_start: Option<NaiveTime>,
    #[serde(default)]
    slot_end: Option<NaiveTime>,
    #[serde(default)]
    acknowledge: bool,
}

async fn update_participation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid, pid)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<UpdateParticipation>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_event(&state, cid, eid).await?;

    // Accuser reception de son line-up est un geste personnel ; le reste est
    // de l'arbitrage, donc admin.
    if !body.acknowledge || body.status.is_some() || body.stage_role.is_some() {
        scope.require_admin()?;
    }

    sqlx::query(
        "UPDATE participations SET
            status = COALESCE($3, status),
            stage_role = COALESCE($4, stage_role),
            slot_start = COALESCE($5, slot_start),
            slot_end = COALESCE($6, slot_end),
            acknowledged_at = CASE WHEN $7 THEN now() ELSE acknowledged_at END
         WHERE id = $1 AND event_id = $2",
    )
    .bind(pid)
    .bind(eid)
    .bind(&body.status)
    .bind(&body.stage_role)
    .bind(body.slot_start)
    .bind(body.slot_end)
    .bind(body.acknowledge)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn remove_participation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid, pid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_event(&state, cid, eid).await?;
    sqlx::query("DELETE FROM participations WHERE id = $1 AND event_id = $2")
        .bind(pid)
        .bind(eid)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn check_event(state: &AppState, cid: Uuid, eid: Uuid) -> AppResult<()> {
    let ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM events WHERE id = $1 AND collective_id = $2")
            .bind(eid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    ok.map(|_| ())
        .ok_or_else(|| AppError::not_found("evenement introuvable"))
}

#[derive(Deserialize)]
struct NewSlot {
    /// Texte libre, sans catalogue (§5.4, §20).
    label: String,
    #[serde(default = "one")]
    quantity: i32,
    #[serde(default)]
    notes: Option<String>,
}

fn one() -> i32 {
    1
}

async fn add_slot(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewSlot>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_event(&state, cid, eid).await?;
    if body.quantity < 1 {
        return Err(AppError::bad_request("quantite minimale : 1"));
    }
    let (pos,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(max(position) + 1, 0) FROM logistics_slots WHERE event_id = $1",
    )
    .bind(eid)
    .fetch_one(&state.db)
    .await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO logistics_slots (event_id, label, quantity, notes, position)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(eid)
    .bind(&body.label)
    .bind(body.quantity)
    .bind(&body.notes)
    .bind(pos)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn remove_slot(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid, sid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_event(&state, cid, eid).await?;
    sqlx::query("DELETE FROM logistics_slots WHERE id = $1 AND event_id = $2")
        .bind(sid)
        .bind(eid)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// « Je le prends » — volontariat, chacun pour soi.
async fn take_slot(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid, sid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_event(&state, cid, eid).await?;

    let slot: Option<(i32,)> =
        sqlx::query_as("SELECT quantity FROM logistics_slots WHERE id = $1 AND event_id = $2")
            .bind(sid)
            .bind(eid)
            .fetch_optional(&state.db)
            .await?;
    let (quantity,) = slot.ok_or_else(|| AppError::not_found("poste introuvable"))?;

    let (taken,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM logistics_assignments WHERE slot_id = $1")
            .bind(sid)
            .fetch_one(&state.db)
            .await?;
    if taken >= quantity as i64 {
        return Err(AppError::conflict("ce poste est complet"));
    }

    sqlx::query(
        "INSERT INTO logistics_assignments (slot_id, user_id) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
    )
    .bind(sid)
    .bind(scope.user_id())
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn release_slot(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid, sid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_event(&state, cid, eid).await?;
    sqlx::query("DELETE FROM logistics_assignments WHERE slot_id = $1 AND user_id = $2")
        .bind(sid)
        .bind(scope.user_id())
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Autocompletion sur les libelles deja utilises dans le collectif (§4).
async fn logistics_label_suggestions(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, _eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<String>>> {
    let _scope = state.scope(actor, cid).await?;
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT s.label FROM logistics_slots s JOIN events e ON e.id = s.event_id
         WHERE e.collective_id = $1
         GROUP BY s.label ORDER BY count(*) DESC, s.label LIMIT 40",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(l,)| l).collect()))
}

#[derive(Deserialize)]
struct SetPresence {
    /// `coming` | `not_coming` | `unsure` — « tu viens ? », pas « quand ? » (§6)
    answer: String,
    /// Facultatif, jamais reclame.
    #[serde(default)]
    days: Vec<NaiveDate>,
}

async fn set_presence(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<SetPresence>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    check_event(&state, cid, eid).await?;
    if !matches!(body.answer.as_str(), "coming" | "not_coming" | "unsure") {
        return Err(AppError::bad_request(
            "reponse attendue : coming | not_coming | unsure",
        ));
    }

    let mut tx = state.db.begin().await?;
    let (pid,): (Uuid,) = sqlx::query_as(
        "INSERT INTO residency_presences (event_id, user_id, answer) VALUES ($1, $2, $3)
         ON CONFLICT (event_id, user_id)
         DO UPDATE SET answer = EXCLUDED.answer, updated_at = now()
         RETURNING id",
    )
    .bind(eid)
    .bind(scope.user_id())
    .bind(&body.answer)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM residency_presence_days WHERE presence_id = $1")
        .bind(pid)
        .execute(&mut *tx)
        .await?;
    for day in &body.days {
        sqlx::query("INSERT INTO residency_presence_days (presence_id, day) VALUES ($1, $2)")
            .bind(pid)
            .bind(day)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct UpdateStream {
    #[serde(default)]
    capture_location: Option<String>,
    #[serde(default)]
    planned_duration_min: Option<i32>,
    /// Renseigne apres coup — il alimente les taches de com J+1 et J+3 (§6).
    #[serde(default)]
    replay_url: Option<String>,
    #[serde(default)]
    platforms: Option<Vec<StreamPlatform>>,
}

async fn update_stream(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateStream>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_event(&state, cid, eid).await?;

    sqlx::query(
        "INSERT INTO event_streams (event_id, capture_location, planned_duration_min, replay_url)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (event_id) DO UPDATE SET
            capture_location = COALESCE($2, event_streams.capture_location),
            planned_duration_min = COALESCE($3, event_streams.planned_duration_min),
            replay_url = COALESCE($4, event_streams.replay_url)",
    )
    .bind(eid)
    .bind(&body.capture_location)
    .bind(body.planned_duration_min)
    .bind(&body.replay_url)
    .execute(&state.db)
    .await?;

    if let Some(platforms) = &body.platforms {
        sqlx::query("DELETE FROM event_stream_platforms WHERE event_id = $1")
            .bind(eid)
            .execute(&state.db)
            .await?;
        for p in platforms {
            sqlx::query(
                "INSERT INTO event_stream_platforms (event_id, platform, url) VALUES ($1, $2, $3)",
            )
            .bind(eid)
            .bind(&p.platform)
            .bind(&p.url)
            .execute(&state.db)
            .await?;
        }
    }

    Ok(Json(json!({ "ok": true })))
}

async fn get_run_sheet(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<run_sheet::RunSheet>> {
    let scope = state.scope(actor, cid).await?;
    let with_phones = run_sheet::may_see_phones(&state.db, &scope, eid).await?;
    Ok(Json(
        run_sheet::build(&state.db, &scope, eid, with_phones).await?,
    ))
}
