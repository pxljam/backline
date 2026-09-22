//! Opportunites, dates candidates, sondage de dispos (§5.1 a §5.3).
//!
//! C'est le coeur du produit : la ou les allers-retours disparaissent.

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{events as events_svc, matrix, notify};
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/:oid", get(show))
        .route("/:oid/dates", post(add_date))
        .route("/:oid/poll", post(open_poll).delete(close_poll))
        .route("/:oid/availabilities", put(set_availabilities))
        .route("/:oid/matrix", get(show_matrix))
        .route("/:oid/convert", post(convert))
}

#[derive(Serialize)]
struct OpportunityRow {
    id: Uuid,
    title: String,
    status: String,
    conditions: Option<String>,
    venue: Option<VenueBrief>,
    host_group_ids: Vec<Uuid>,
    candidate_dates: Vec<CandidateDate>,
    poll_open: bool,
    /// Presente une fois la date arretee.
    event_id: Option<Uuid>,
}

#[derive(Serialize)]
struct VenueBrief {
    id: Uuid,
    name: String,
    city: Option<String>,
}

#[derive(Serialize)]
struct CandidateDate {
    id: Uuid,
    day: NaiveDate,
    start_time: Option<chrono::NaiveTime>,
    end_time: Option<chrono::NaiveTime>,
    notes: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<OpportunityRow>>> {
    let _scope = state.scope(actor, cid).await?;
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM opportunities WHERE collective_id = $1 ORDER BY created_at DESC",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for (id,) in rows {
        out.push(load(&state, cid, id).await?);
    }
    Ok(Json(out))
}

async fn load(state: &AppState, cid: Uuid, oid: Uuid) -> AppResult<OpportunityRow> {
    let row: Option<(Uuid, String, String, Option<String>, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, title, status, conditions, venue_id FROM opportunities
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(oid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (id, title, status, conditions, venue_id) =
        row.ok_or_else(|| AppError::not_found("opportunite introuvable"))?;

    let venue = match venue_id {
        Some(vid) => {
            let v: (Uuid, String, Option<String>) =
                sqlx::query_as("SELECT id, name, city FROM venues WHERE id = $1")
                    .bind(vid)
                    .fetch_one(&state.db)
                    .await?;
            Some(VenueBrief {
                id: v.0,
                name: v.1,
                city: v.2,
            })
        }
        None => None,
    };

    let hosts: Vec<(Uuid,)> =
        sqlx::query_as("SELECT group_id FROM opportunity_hosts WHERE opportunity_id = $1")
            .bind(oid)
            .fetch_all(&state.db)
            .await?;

    let dates: Vec<(
        Uuid,
        NaiveDate,
        Option<chrono::NaiveTime>,
        Option<chrono::NaiveTime>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, day, start_time, end_time, notes FROM candidate_dates
             WHERE opportunity_id = $1 ORDER BY position, day",
    )
    .bind(oid)
    .fetch_all(&state.db)
    .await?;

    let poll: Option<(Option<chrono::DateTime<chrono::Utc>>,)> =
        sqlx::query_as("SELECT closed_at FROM availability_polls WHERE opportunity_id = $1")
            .bind(oid)
            .fetch_optional(&state.db)
            .await?;

    let event: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM events WHERE opportunity_id = $1 LIMIT 1")
            .bind(oid)
            .fetch_optional(&state.db)
            .await?;

    Ok(OpportunityRow {
        id,
        title,
        status,
        conditions,
        venue,
        host_group_ids: hosts.into_iter().map(|(g,)| g).collect(),
        candidate_dates: dates
            .into_iter()
            .map(|(id, day, start_time, end_time, notes)| CandidateDate {
                id,
                day,
                start_time,
                end_time,
                notes,
            })
            .collect(),
        poll_open: matches!(poll, Some((None,))),
        event_id: event.map(|(e,)| e),
    })
}

async fn show(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<OpportunityRow>> {
    let _scope = state.scope(actor, cid).await?;
    Ok(Json(load(&state, cid, oid).await?))
}

#[derive(Deserialize)]
struct NewOpportunity {
    title: String,
    #[serde(default)]
    venue_id: Option<Uuid>,
    #[serde(default)]
    venue_contact_id: Option<Uuid>,
    #[serde(default)]
    conditions: Option<String>,
    #[serde(default)]
    host_group_ids: Vec<Uuid>,
    /// N dates candidates proposees par le lieu (§5.1).
    #[serde(default)]
    candidate_dates: Vec<NewCandidateDate>,
}

#[derive(Deserialize)]
struct NewCandidateDate {
    day: NaiveDate,
    #[serde(default)]
    start_time: Option<chrono::NaiveTime>,
    #[serde(default)]
    end_time: Option<chrono::NaiveTime>,
    #[serde(default)]
    notes: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewOpportunity>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    if let Some(vid) = body.venue_id {
        let ok: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM venues WHERE id = $1 AND collective_id = $2")
                .bind(vid)
                .bind(cid)
                .fetch_optional(&state.db)
                .await?;
        ok.ok_or_else(|| AppError::bad_request("lieu hors du collectif"))?;
    }

    let mut tx = state.db.begin().await?;
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO opportunities
            (collective_id, venue_id, venue_contact_id, title, conditions, created_by)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(cid)
    .bind(body.venue_id)
    .bind(body.venue_contact_id)
    .bind(&body.title)
    .bind(&body.conditions)
    .bind(scope.user_id())
    .fetch_one(&mut *tx)
    .await?;

    for gid in &body.host_group_ids {
        scope.check_group(&state.db, *gid).await?;
        sqlx::query("INSERT INTO opportunity_hosts (opportunity_id, group_id) VALUES ($1, $2)")
            .bind(id)
            .bind(gid)
            .execute(&mut *tx)
            .await?;
    }

    for (i, d) in body.candidate_dates.iter().enumerate() {
        sqlx::query(
            "INSERT INTO candidate_dates (opportunity_id, day, start_time, end_time, notes, position)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(d.day)
        .bind(d.start_time)
        .bind(d.end_time)
        .bind(&d.notes)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(Json(json!({ "id": id })))
}

async fn add_date(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewCandidateDate>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_opportunity(&state, cid, oid).await?;

    let (pos,): (i32,) = sqlx::query_as(
        "SELECT COALESCE(max(position) + 1, 0) FROM candidate_dates WHERE opportunity_id = $1",
    )
    .bind(oid)
    .fetch_one(&state.db)
    .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO candidate_dates (opportunity_id, day, start_time, end_time, notes, position)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
    )
    .bind(oid)
    .bind(body.day)
    .bind(body.start_time)
    .bind(body.end_time)
    .bind(&body.notes)
    .bind(pos)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}

async fn check_opportunity(state: &AppState, cid: Uuid, oid: Uuid) -> AppResult<()> {
    let ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM opportunities WHERE id = $1 AND collective_id = $2")
            .bind(oid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    ok.map(|_| ())
        .ok_or_else(|| AppError::not_found("opportunite introuvable"))
}

/// Ouvre le sondage : un message par opportunite a chaque membre concerne,
/// une ligne par date candidate (§5.2).
async fn open_poll(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_opportunity(&state, cid, oid).await?;

    let (dates,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM candidate_dates WHERE opportunity_id = $1")
            .bind(oid)
            .fetch_one(&state.db)
            .await?;
    if dates == 0 {
        return Err(AppError::bad_request(
            "ajouter au moins une date candidate avant d'ouvrir le sondage",
        ));
    }

    let (poll_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO availability_polls (opportunity_id, opened_by) VALUES ($1, $2)
         ON CONFLICT (opportunity_id) DO UPDATE SET closed_at = NULL, opened_at = now()
         RETURNING id",
    )
    .bind(oid)
    .bind(scope.user_id())
    .fetch_one(&state.db)
    .await?;

    sqlx::query("UPDATE opportunities SET status = 'poll_open', updated_at = now() WHERE id = $1")
        .bind(oid)
        .execute(&state.db)
        .await?;

    let (title,): (String,) = sqlx::query_as("SELECT title FROM opportunities WHERE id = $1")
        .bind(oid)
        .fetch_one(&state.db)
        .await?;

    for user_id in notify::collective_member_ids(&state.db, cid).await? {
        notify::push(
            &state.db,
            notify::Notice {
                user_id,
                collective_id: Some(cid),
                kind: "poll_open",
                title: format!("Dispos demandees : {title}"),
                body: format!("{dates} dates candidates. ✅ dispo / ❔ peut-etre / ❌ non, et 🎸 si tu veux jouer."),
                payload: json!({ "opportunity_id": oid }),
            },
        )
        .await?;
    }

    Ok(Json(json!({ "poll_id": poll_id })))
}

async fn close_poll(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    check_opportunity(&state, cid, oid).await?;
    sqlx::query("UPDATE availability_polls SET closed_at = now() WHERE opportunity_id = $1")
        .bind(oid)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct AvailabilityInput {
    candidate_date_id: Uuid,
    status: String,
    #[serde(default)]
    wants_to_play: bool,
}

#[derive(Deserialize)]
struct SetAvailabilities {
    /// Toujours present, toujours egal a l'appelant. Le champ existe pour que
    /// la tentative d'ecrire pour autrui soit **explicite et refusee**, plutot
    /// que silencieusement reinterpretee.
    #[serde(default)]
    user_id: Option<Uuid>,
    answers: Vec<AvailabilityInput>,
}

/// Une disponibilite **n'est modifiable que par la personne concernee**,
/// admins compris (§5.2, §20).
async fn set_availabilities(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
    Json(body): Json<SetAvailabilities>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    if let Some(target) = body.user_id {
        scope.require_self(target)?;
    }
    check_opportunity(&state, cid, oid).await?;

    let poll: Option<(Uuid, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as("SELECT id, closed_at FROM availability_polls WHERE opportunity_id = $1")
            .bind(oid)
            .fetch_optional(&state.db)
            .await?;
    let (poll_id, closed_at) =
        poll.ok_or_else(|| AppError::bad_request("le sondage n'est pas ouvert"))?;
    if closed_at.is_some() {
        return Err(AppError::conflict("le sondage est clos"));
    }

    let mut tx = state.db.begin().await?;
    for a in &body.answers {
        if !matches!(a.status.as_str(), "yes" | "maybe" | "no") {
            return Err(AppError::bad_request("statut attendu : yes | maybe | no"));
        }
        // La date doit appartenir a cette opportunite : sinon on ecrirait dans
        // le sondage d'un autre collectif.
        let ok: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM candidate_dates WHERE id = $1 AND opportunity_id = $2")
                .bind(a.candidate_date_id)
                .bind(oid)
                .fetch_optional(&mut *tx)
                .await?;
        ok.ok_or_else(|| AppError::bad_request("date candidate hors de cette opportunite"))?;

        sqlx::query(
            "INSERT INTO availabilities (poll_id, candidate_date_id, user_id, status, wants_to_play)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (candidate_date_id, user_id)
             DO UPDATE SET status = EXCLUDED.status,
                           wants_to_play = EXCLUDED.wants_to_play,
                           updated_at = now()",
        )
        .bind(poll_id)
        .bind(a.candidate_date_id)
        .bind(scope.user_id())
        .bind(&a.status)
        .bind(a.wants_to_play)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(Json(json!({ "ok": true, "count": body.answers.len() })))
}

/// Les disponibilites sont **visibles par tous les membres** du collectif (§5.2).
async fn show_matrix(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<matrix::Matrix>> {
    let scope = state.scope(actor, cid).await?;
    Ok(Json(matrix::build(&state.db, &scope, oid).await?))
}

async fn convert(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, oid)): Path<(Uuid, Uuid)>,
    Json(body): Json<events_svc::ConvertRequest>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    let event_id = events_svc::convert(&state.db, &scope, oid, body).await?;
    Ok(Json(json!({ "event_id": event_id })))
}
