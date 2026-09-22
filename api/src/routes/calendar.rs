//! Unified calendar and iCal feeds (§7).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::ical;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(calendar))
        .route("/feeds", get(feeds))
}

/// Public route, outside `/api`: this is the URL pasted into Google Calendar.
/// The secret token **is** the authentication.
pub fn public_router() -> Router<AppState> {
    Router::new().route("/:token", get(ical_feed))
}

#[derive(Deserialize)]
struct CalendarQuery {
    #[serde(default)]
    from: Option<DateTime<Utc>>,
    #[serde(default)]
    to: Option<DateTime<Utc>>,
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    mine: Option<bool>,
    /// Includes candidate dates still under arbitration (shown dotted).
    #[serde(default = "yes")]
    candidates: bool,
}

fn yes() -> bool {
    true
}

#[derive(Serialize)]
struct CalendarEntry {
    id: Uuid,
    kind: String,
    title: String,
    status: String,
    starts_at: DateTime<Utc>,
    ends_at: Option<DateTime<Utc>>,
    venue: Option<String>,
    city: Option<String>,
    type_key: Option<String>,
    opportunity_id: Option<Uuid>,
}

async fn calendar(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Query(q): Query<CalendarQuery>,
) -> AppResult<Json<Vec<CalendarEntry>>> {
    let scope = state.scope(actor, cid).await?;

    #[allow(clippy::type_complexity)]
    let events: Vec<(
        Uuid,
        String,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT e.id, e.title, e.status, e.starts_at, e.ends_at, v.name, v.city, t.key
             FROM events e
             JOIN event_types t ON t.id = e.event_type_id
             LEFT JOIN venues v ON v.id = e.venue_id
             WHERE e.collective_id = $1
               AND ($2::timestamptz IS NULL OR e.starts_at >= $2)
               AND ($3::timestamptz IS NULL OR e.starts_at <= $3)
               AND ($4::uuid IS NULL OR e.host_group_id = $4 OR EXISTS (
                     SELECT 1 FROM participations p WHERE p.event_id = e.id AND p.group_id = $4))
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
    .bind(q.from)
    .bind(q.to)
    .bind(q.group_id)
    .bind(q.mine)
    .bind(scope.user_id())
    .fetch_all(&state.db)
    .await?;

    let mut out: Vec<CalendarEntry> = events
        .into_iter()
        .map(
            |(id, title, status, starts_at, ends_at, venue, city, type_key)| CalendarEntry {
                id,
                kind: "event".into(),
                title,
                status,
                starts_at,
                ends_at,
                venue,
                city,
                type_key: Some(type_key),
                opportunity_id: None,
            },
        )
        .collect();

    // Candidate dates still under arbitration: they take up room in the
    // calendar without being commitments.
    if q.candidates && q.group_id.is_none() {
        let rows: Vec<(Uuid, Uuid, String, NaiveDate, Option<chrono::NaiveTime>, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT d.id, o.id, o.title, d.day, d.start_time, v.name, v.city
                 FROM candidate_dates d
                 JOIN opportunities o ON o.id = d.opportunity_id
                 LEFT JOIN venues v ON v.id = o.venue_id
                 WHERE o.collective_id = $1 AND o.status IN ('discussing', 'poll_open', 'date_chosen')
                 ORDER BY d.day",
            )
            .bind(cid)
            .fetch_all(&state.db)
            .await?;

        for (id, opp_id, title, day, start_time, venue, city) in rows {
            let starts_at = crate::services::events::local_datetime(day, start_time);
            if q.from.is_some_and(|f| starts_at < f) || q.to.is_some_and(|t| starts_at > t) {
                continue;
            }
            out.push(CalendarEntry {
                id,
                kind: "candidate_date".into(),
                title,
                status: "tentative".into(),
                starts_at,
                ends_at: None,
                venue,
                city,
                type_key: None,
                opportunity_id: Some(opp_id),
            });
        }
        out.sort_by_key(|e| e.starts_at);
    }

    Ok(Json(out))
}

#[derive(Serialize)]
struct Feed {
    scope: String,
    scope_id: Uuid,
    label: String,
    url: String,
}

/// One feed per member, one per group, one per collective (§7).
async fn feeds(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Feed>>> {
    let scope = state.scope(actor, cid).await?;
    let base = &state.config.public_base_url;
    let mut out = Vec::new();

    let (name,): (String,) = sqlx::query_as("SELECT name FROM collectives WHERE id = $1")
        .bind(cid)
        .fetch_one(&state.db)
        .await?;
    let token = ical::ensure_token(&state.db, "collective", cid).await?;
    out.push(Feed {
        scope: "collective".into(),
        scope_id: cid,
        label: name,
        url: format!("{base}/ical/{token}.ics"),
    });

    // Personal feed: only its owner receives the URL.
    let token = ical::ensure_token(&state.db, "user", scope.user_id()).await?;
    out.push(Feed {
        scope: "user".into(),
        scope_id: scope.user_id(),
        label: "Mes evenements".into(),
        url: format!("{base}/ical/{token}.ics"),
    });

    let groups: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT g.id, g.name FROM group_members gm JOIN groups g ON g.id = gm.group_id
         WHERE gm.user_id = $1 AND g.collective_id = $2",
    )
    .bind(scope.user_id())
    .bind(cid)
    .fetch_all(&state.db)
    .await?;
    for (gid, gname) in groups {
        let token = ical::ensure_token(&state.db, "group", gid).await?;
        out.push(Feed {
            scope: "group".into(),
            scope_id: gid,
            label: gname,
            url: format!("{base}/ical/{token}.ics"),
        });
    }

    Ok(Json(out))
}

/// A member sees their events in their calendar **without having signed in to
/// anything but the bot** (§18).
async fn ical_feed(
    State(state): State<AppState>,
    Path(token): Path<String>,
) -> AppResult<impl IntoResponse> {
    let token = token.trim_end_matches(".ics");
    let row: Option<(String, Uuid)> =
        sqlx::query_as("SELECT scope, scope_id FROM ical_tokens WHERE token = $1")
            .bind(token)
            .fetch_optional(&state.db)
            .await?;
    let (scope, scope_id) = row.ok_or_else(|| AppError::not_found("flux inconnu"))?;

    #[allow(clippy::type_complexity)]
    let (label, rows): (
        String,
        Vec<(
            Uuid,
            String,
            DateTime<Utc>,
            Option<DateTime<Utc>>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )>,
    ) = match scope.as_str() {
        "collective" => {
            let (name,): (String,) = sqlx::query_as("SELECT name FROM collectives WHERE id = $1")
                .bind(scope_id)
                .fetch_one(&state.db)
                .await?;
            let rows = sqlx::query_as(
                "SELECT e.id, e.title, e.starts_at, e.ends_at, e.status, v.name, v.city, e.notes
                     FROM events e LEFT JOIN venues v ON v.id = e.venue_id
                     WHERE e.collective_id = $1 AND e.status <> 'draft' ORDER BY e.starts_at",
            )
            .bind(scope_id)
            .fetch_all(&state.db)
            .await?;
            (name, rows)
        }
        "group" => {
            let (name,): (String,) = sqlx::query_as("SELECT name FROM groups WHERE id = $1")
                .bind(scope_id)
                .fetch_one(&state.db)
                .await?;
            let rows = sqlx::query_as(
                    "SELECT DISTINCT e.id, e.title, e.starts_at, e.ends_at, e.status, v.name, v.city, e.notes
                     FROM events e
                     LEFT JOIN venues v ON v.id = e.venue_id
                     LEFT JOIN participations p ON p.event_id = e.id
                     WHERE (e.host_group_id = $1 OR p.group_id = $1) AND e.status <> 'draft'
                     ORDER BY e.starts_at",
                )
                .bind(scope_id)
                .fetch_all(&state.db)
                .await?;
            (name, rows)
        }
        _ => {
            let (name,): (String,) = sqlx::query_as("SELECT display_name FROM users WHERE id = $1")
                .bind(scope_id)
                .fetch_one(&state.db)
                .await?;
            let rows = sqlx::query_as(
                    "SELECT DISTINCT e.id, e.title, e.starts_at, e.ends_at, e.status, v.name, v.city, e.notes
                     FROM events e
                     LEFT JOIN venues v ON v.id = e.venue_id
                     LEFT JOIN participations p ON p.event_id = e.id
                     LEFT JOIN group_members gm ON gm.group_id = p.group_id
                     LEFT JOIN logistics_slots ls ON ls.event_id = e.id
                     LEFT JOIN logistics_assignments la ON la.slot_id = ls.id
                     WHERE e.status <> 'draft'
                       AND (p.user_id = $1 OR gm.user_id = $1 OR la.user_id = $1)
                     ORDER BY e.starts_at",
                )
                .bind(scope_id)
                .fetch_all(&state.db)
                .await?;
            (format!("Backline — {name}"), rows)
        }
    };

    let events: Vec<ical::IcalEvent> = rows
        .into_iter()
        .map(
            |(id, title, starts_at, ends_at, status, venue, city, notes)| ical::IcalEvent {
                id,
                title,
                starts_at,
                ends_at,
                status,
                venue,
                city,
                notes,
                tentative: false,
            },
        )
        .collect();

    Ok((
        [
            (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=300"),
        ],
        ical::render(&label, &events),
    ))
}
