//! Venue address book (§14).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/:venue_id", get(show))
        .route("/:venue_id/contacts", post(add_contact))
}

#[derive(Serialize)]
pub struct VenueRow {
    pub id: Uuid,
    pub name: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub capacity: Option<i32>,
    pub notes: Option<String>,
    pub contacts: Vec<ContactRow>,
    pub past_events: i64,
}

#[derive(Serialize)]
pub struct ContactRow {
    pub id: Uuid,
    pub name: String,
    pub role: Option<String>,
    pub phone: Option<String>,
    pub email: Option<String>,
}

async fn list(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<VenueRow>>> {
    let _scope = state.scope(actor, cid).await?;
    let venues: Vec<(
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        "SELECT v.id, v.name, v.address, v.city, v.capacity, v.notes,
                    (SELECT count(*) FROM events e WHERE e.venue_id = v.id)
             FROM venues v WHERE v.collective_id = $1 ORDER BY v.name",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for (id, name, address, city, capacity, notes, past_events) in venues {
        out.push(VenueRow {
            id,
            name,
            address,
            city,
            capacity,
            notes,
            contacts: contacts_of(&state, id).await?,
            past_events,
        });
    }
    Ok(Json(out))
}

async fn contacts_of(state: &AppState, venue_id: Uuid) -> AppResult<Vec<ContactRow>> {
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, name, role, phone, email FROM venue_contacts WHERE venue_id = $1 ORDER BY name",
    )
    .bind(venue_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, role, phone, email)| ContactRow {
            id,
            name,
            role,
            phone,
            email,
        })
        .collect())
}

async fn show(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, vid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<VenueRow>> {
    let _scope = state.scope(actor, cid).await?;
    let row: Option<(
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<String>,
        i64,
    )> = sqlx::query_as(
        "SELECT v.id, v.name, v.address, v.city, v.capacity, v.notes,
                    (SELECT count(*) FROM events e WHERE e.venue_id = v.id)
             FROM venues v WHERE v.id = $1 AND v.collective_id = $2",
    )
    .bind(vid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (id, name, address, city, capacity, notes, past_events) =
        row.ok_or_else(|| AppError::not_found("lieu introuvable"))?;
    Ok(Json(VenueRow {
        id,
        name,
        address,
        city,
        capacity,
        notes,
        contacts: contacts_of(&state, vid).await?,
        past_events,
    }))
}

#[derive(Deserialize)]
struct NewVenue {
    name: String,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    capacity: Option<i32>,
    #[serde(default)]
    notes: Option<String>,
}

async fn create(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewVenue>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO venues (collective_id, name, address, city, country, capacity, notes)
         VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
    )
    .bind(cid)
    .bind(&body.name)
    .bind(&body.address)
    .bind(&body.city)
    .bind(&body.country)
    .bind(body.capacity)
    .bind(&body.notes)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct NewContact {
    name: String,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    email: Option<String>,
}

async fn add_contact(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, vid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewContact>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    let ok: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM venues WHERE id = $1 AND collective_id = $2")
            .bind(vid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    ok.ok_or_else(|| AppError::not_found("lieu introuvable"))?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO venue_contacts (venue_id, name, role, phone, email)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(vid)
    .bind(&body.name)
    .bind(&body.role)
    .bind(&body.phone)
    .bind(&body.email)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "id": id })))
}
