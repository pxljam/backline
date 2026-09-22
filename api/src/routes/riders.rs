//! Fiches techniques versionnees, press kit, comptes sociaux (§11.3, §12).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::pdf;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/groups/:group_id/tech-riders",
            get(list_riders).post(create_rider),
        )
        .route(
            "/groups/:group_id/tech-riders/:rider_id",
            get(show_rider).patch(update_rider),
        )
        .route(
            "/groups/:group_id/tech-riders/:rider_id/publish",
            post(publish_rider),
        )
        .route(
            "/groups/:group_id/tech-riders/:rider_id/pdf",
            get(rider_pdf),
        )
        .route(
            "/groups/:group_id/press-kit",
            get(show_press_kit).put(save_press_kit),
        )
        .route("/social-accounts", get(list_accounts).post(create_account))
        .route("/social-accounts/:account_id/access", put(set_access))
        .route("/events/:event_id/tech-riders/send", post(send_riders))
}

#[derive(Serialize)]
struct RiderRow {
    id: Uuid,
    version: i32,
    status: String,
    data: Value,
    created_at: DateTime<Utc>,
}

async fn list_riders(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Vec<RiderRow>>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    let rows: Vec<(Uuid, i32, String, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, version, status, data, created_at FROM tech_riders
         WHERE group_id = $1 ORDER BY version DESC",
    )
    .bind(gid)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, version, status, data, created_at)| RiderRow {
                id,
                version,
                status,
                data,
                created_at,
            })
            .collect(),
    ))
}

async fn show_rider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<RiderRow>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    let row: Option<(Uuid, i32, String, Value, DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, version, status, data, created_at FROM tech_riders
         WHERE id = $1 AND group_id = $2",
    )
    .bind(rid)
    .bind(gid)
    .fetch_optional(&state.db)
    .await?;
    let (id, version, status, data, created_at) =
        row.ok_or_else(|| AppError::not_found("fiche technique introuvable"))?;
    Ok(Json(RiderRow {
        id,
        version,
        status,
        data,
        created_at,
    }))
}

#[derive(Deserialize)]
struct NewRider {
    #[serde(default)]
    data: Value,
    /// Repart d'une version existante plutot que d'une page blanche.
    #[serde(default)]
    from_version: Option<i32>,
}

/// Une fiche technique est **versionnee** : creer une nouvelle version ne
/// touche jamais celle qu'un lieu a deja recue (§4).
async fn create_rider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewRider>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;

    let base = match body.from_version {
        Some(v) => sqlx::query_as::<_, (Value,)>(
            "SELECT data FROM tech_riders WHERE group_id = $1 AND version = $2",
        )
        .bind(gid)
        .bind(v)
        .fetch_optional(&state.db)
        .await?
        .map(|(d,)| d)
        .unwrap_or(json!({})),
        None => json!({}),
    };

    let data = if body.data.is_null() || body.data == json!({}) {
        base
    } else {
        body.data
    };

    let (next,): (i32,) =
        sqlx::query_as("SELECT COALESCE(max(version) + 1, 1) FROM tech_riders WHERE group_id = $1")
            .bind(gid)
            .fetch_one(&state.db)
            .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO tech_riders (group_id, version, status, data, created_by)
         VALUES ($1, $2, 'draft', $3, $4) RETURNING id",
    )
    .bind(gid)
    .bind(next)
    .bind(&data)
    .bind(scope.user_id())
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({ "id": id, "version": next })))
}

#[derive(Deserialize)]
struct UpdateRider {
    data: Value,
}

async fn update_rider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid, rid)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<UpdateRider>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;

    // Une version publiee est figee : c'est ce qui garantit qu'un PDF deja
    // envoye reste reproductible.
    let row: Option<(String,)> =
        sqlx::query_as("SELECT status FROM tech_riders WHERE id = $1 AND group_id = $2")
            .bind(rid)
            .bind(gid)
            .fetch_optional(&state.db)
            .await?;
    let (status,) = row.ok_or_else(|| AppError::not_found("fiche technique introuvable"))?;
    if status == "published" {
        return Err(AppError::conflict(
            "cette version est publiee — en creer une nouvelle pour la modifier",
        ));
    }

    sqlx::query("UPDATE tech_riders SET data = $2 WHERE id = $1")
        .bind(rid)
        .bind(&body.data)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn publish_rider(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;
    sqlx::query("UPDATE tech_riders SET status = 'published' WHERE id = $1 AND group_id = $2")
        .bind(rid)
        .bind(gid)
        .execute(&state.db)
        .await?;

    // Les evenements a venir qui n'avaient pas de fiche en recoivent une.
    sqlx::query(
        "UPDATE event_tech_riders etr SET tech_rider_id = $1
         FROM events e
         WHERE etr.event_id = e.id AND etr.group_id = $2
           AND etr.tech_rider_id IS NULL AND etr.sent_at IS NULL AND e.starts_at > now()",
    )
    .bind(rid)
    .bind(gid)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "ok": true })))
}

/// Le PDF part vers un lieu **en moins d'une minute** (§18) : une requete,
/// une compilation Typst, un fichier.
async fn rider_pdf(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid, rid)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<impl IntoResponse> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;

    let row: Option<(i32, Value, String)> = sqlx::query_as(
        "SELECT r.version, r.data, g.name FROM tech_riders r JOIN groups g ON g.id = r.group_id
         WHERE r.id = $1 AND r.group_id = $2",
    )
    .bind(rid)
    .bind(gid)
    .fetch_optional(&state.db)
    .await?;
    let (version, data, group_name) =
        row.ok_or_else(|| AppError::not_found("fiche technique introuvable"))?;

    let payload = json!({
        "group_name": group_name,
        "version": version,
        "rider": data,
        "generated_on": Utc::now().format("%d/%m/%Y").to_string(),
    });

    let bytes = pdf::compile("tech-rider.typ", &payload).await?;
    let filename = format!("fiche-technique-{}-v{version}.pdf", slug(&group_name));

    Ok((
        [
            (header::CONTENT_TYPE, "application/pdf".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        bytes,
    ))
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

#[derive(Deserialize)]
struct SendRiders {
    #[serde(default)]
    sent_to: Option<String>,
}

/// Marque l'envoi au lieu et **liste explicitement les groupes sans fiche**,
/// pour que l'admin sache ce qu'il n'envoie pas (§12).
async fn send_riders(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
    Json(body): Json<SendRiders>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    crate::routes::events::check_event(&state, cid, eid).await?;

    let rows: Vec<(Uuid, String, Option<Uuid>, Option<i32>)> = sqlx::query_as(
        "SELECT etr.group_id, g.name, etr.tech_rider_id, r.version
         FROM event_tech_riders etr
         JOIN groups g ON g.id = etr.group_id
         LEFT JOIN tech_riders r ON r.id = etr.tech_rider_id
         WHERE etr.event_id = $1",
    )
    .bind(eid)
    .fetch_all(&state.db)
    .await?;

    let mut sent = Vec::new();
    let mut missing = Vec::new();
    for (group_id, name, rider, version) in rows {
        match rider {
            Some(_) => {
                sqlx::query(
                    "UPDATE event_tech_riders SET sent_at = now(), sent_to = $3
                     WHERE event_id = $1 AND group_id = $2",
                )
                .bind(eid)
                .bind(group_id)
                .bind(&body.sent_to)
                .execute(&state.db)
                .await?;
                sent.push(json!({ "group_id": group_id, "name": name, "version": version }));
            }
            // Rien n'est bloque : l'absence est signalee, jamais une barriere.
            None => missing.push(json!({ "group_id": group_id, "name": name })),
        }
    }

    Ok(Json(json!({ "sent": sent, "missing": missing })))
}

#[derive(Serialize, Deserialize)]
struct PressKit {
    #[serde(default)]
    bio_short: Option<String>,
    #[serde(default)]
    bio_long: Option<String>,
    #[serde(default)]
    links: Value,
    #[serde(default)]
    photo_asset_ids: Vec<Uuid>,
}

async fn show_press_kit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<PressKit>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    let row: Option<(Option<String>, Option<String>, Value, Vec<Uuid>)> = sqlx::query_as(
        "SELECT bio_short, bio_long, links, photo_asset_ids FROM press_kits WHERE group_id = $1",
    )
    .bind(gid)
    .fetch_optional(&state.db)
    .await?;
    let (bio_short, bio_long, links, photo_asset_ids) =
        row.unwrap_or((None, None, json!([]), vec![]));
    Ok(Json(PressKit {
        bio_short,
        bio_long,
        links,
        photo_asset_ids,
    }))
}

async fn save_press_kit(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
    Json(body): Json<PressKit>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;
    sqlx::query(
        "INSERT INTO press_kits (group_id, bio_short, bio_long, links, photo_asset_ids)
         VALUES ($1, $2, $3, $4, $5)
         ON CONFLICT (group_id) DO UPDATE SET
            bio_short = EXCLUDED.bio_short, bio_long = EXCLUDED.bio_long,
            links = EXCLUDED.links, photo_asset_ids = EXCLUDED.photo_asset_ids,
            updated_at = now()",
    )
    .bind(gid)
    .bind(&body.bio_short)
    .bind(&body.bio_long)
    .bind(&body.links)
    .bind(&body.photo_asset_ids)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize)]
struct AccountRow {
    id: Uuid,
    group_id: Option<Uuid>,
    platform: String,
    handle: String,
    url: Option<String>,
    mode: String,
    vault_url: Option<String>,
    notes: Option<String>,
    /// La seule question qui bloque le jour J : qui peut publier ici ?
    access: Vec<Uuid>,
}

async fn list_accounts(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<AccountRow>>> {
    let _scope = state.scope(actor, cid).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        Uuid,
        Option<Uuid>,
        String,
        String,
        Option<String>,
        String,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, group_id, platform, handle, url, mode, vault_url, notes
             FROM social_accounts WHERE collective_id = $1 ORDER BY group_id NULLS FIRST, platform",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let access: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT saa.social_account_id, saa.user_id FROM social_account_access saa
         JOIN social_accounts sa ON sa.id = saa.social_account_id
         WHERE sa.collective_id = $1",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(id, group_id, platform, handle, url, mode, vault_url, notes)| AccountRow {
                    id,
                    group_id,
                    platform,
                    handle,
                    url,
                    mode,
                    vault_url,
                    notes,
                    access: access
                        .iter()
                        .filter(|(a, _)| *a == id)
                        .map(|(_, u)| *u)
                        .collect(),
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewAccount {
    platform: String,
    handle: String,
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    mode: Option<String>,
    /// Lien vers le gestionnaire de mots de passe externe. **Jamais le secret
    /// lui-meme** : l'app n'est pas un coffre-fort (§11.3).
    #[serde(default)]
    vault_url: Option<String>,
    #[serde(default)]
    notes: Option<String>,
    #[serde(default)]
    access: Vec<Uuid>,
}

async fn create_account(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewAccount>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    match body.group_id {
        Some(gid) => {
            scope.check_group(&state.db, gid).await?;
            scope.require_group_admin(&state.db, gid).await?;
        }
        None => scope.require_admin()?,
    }

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO social_accounts (collective_id, group_id, platform, handle, url, mode, vault_url, notes)
         VALUES ($1, $2, $3, $4, $5, COALESCE($6, 'shared'), $7, $8) RETURNING id",
    )
    .bind(cid)
    .bind(body.group_id)
    .bind(&body.platform)
    .bind(&body.handle)
    .bind(&body.url)
    .bind(&body.mode)
    .bind(&body.vault_url)
    .bind(&body.notes)
    .fetch_one(&state.db)
    .await?;

    for user_id in &body.access {
        sqlx::query(
            "INSERT INTO social_account_access (social_account_id, user_id) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(id)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    }
    Ok(Json(json!({ "id": id })))
}

#[derive(Deserialize)]
struct SetAccess {
    access: Vec<Uuid>,
}

async fn set_access(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, aid)): Path<(Uuid, Uuid)>,
    Json(body): Json<SetAccess>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    let row: Option<(Option<Uuid>,)> =
        sqlx::query_as("SELECT group_id FROM social_accounts WHERE id = $1 AND collective_id = $2")
            .bind(aid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    let (group_id,) = row.ok_or_else(|| AppError::not_found("compte introuvable"))?;
    match group_id {
        Some(gid) => scope.require_group_admin(&state.db, gid).await?,
        None => scope.require_admin()?,
    }

    let mut tx = state.db.begin().await?;
    sqlx::query("DELETE FROM social_account_access WHERE social_account_id = $1")
        .bind(aid)
        .execute(&mut *tx)
        .await?;
    for user_id in &body.access {
        sqlx::query(
            "INSERT INTO social_account_access (social_account_id, user_id) VALUES ($1, $2)",
        )
        .bind(aid)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    }

    // Une tache assignee a quelqu'un qui perd l'acces redevient sans
    // responsable : l'invariant du §18 ne se contente pas d'etre verifie a
    // l'assignation.
    sqlx::query(
        "UPDATE publication_tasks SET assignee_id = NULL, status = 'ready', updated_at = now()
         WHERE social_account_id = $1 AND status <> 'published' AND assignee_id IS NOT NULL
           AND assignee_id NOT IN (SELECT user_id FROM social_account_access WHERE social_account_id = $1)",
    )
    .bind(aid)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}
