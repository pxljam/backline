//! Membres et groupes d'un collectif (§3, §14).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::provisioning;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(show))
        .route("/members", get(list_members).post(create_member))
        .route("/members/:user_id", axum::routing::patch(update_member))
        .route("/members/:user_id/invitation", post(regenerate_invitation))
        .route("/groups", get(list_groups).post(create_group))
        .route("/groups/:group_id", get(show_group).patch(update_group))
        .route("/groups/:group_id/members", post(add_group_member))
        .route(
            "/event-types/:type_id",
            axum::routing::patch(update_event_type),
        )
        .route(
            "/groups/:group_id/members/:user_id",
            axum::routing::delete(remove_group_member),
        )
}

#[derive(Serialize)]
struct CollectiveDetail {
    id: Uuid,
    slug: String,
    name: String,
    role: String,
    event_types: Vec<EventTypeRow>,
}

#[derive(Serialize)]
pub struct EventTypeRow {
    pub id: Uuid,
    pub key: String,
    pub label: String,
    pub requires_venue: bool,
    pub is_range: bool,
    pub comms_milestones: serde_json::Value,
    pub default_logistics_slots: serde_json::Value,
}

async fn show(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<CollectiveDetail>> {
    let scope = state.scope(actor, cid).await?;
    let (slug, name): (String, String) =
        sqlx::query_as("SELECT slug, name FROM collectives WHERE id = $1")
            .bind(cid)
            .fetch_one(&state.db)
            .await?;

    #[allow(clippy::type_complexity)]
    let types: Vec<(
        Uuid,
        String,
        String,
        bool,
        bool,
        serde_json::Value,
        serde_json::Value,
    )> = sqlx::query_as(
        "SELECT id, key, label, requires_venue, is_range, comms_milestones,
                default_logistics_slots
         FROM event_types WHERE collective_id = $1 ORDER BY position",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(CollectiveDetail {
        id: cid,
        slug,
        name,
        role: scope.role.as_str().into(),
        event_types: types
            .into_iter()
            .map(
                |(
                    id,
                    key,
                    label,
                    requires_venue,
                    is_range,
                    comms_milestones,
                    default_logistics_slots,
                )| EventTypeRow {
                    id,
                    key,
                    label,
                    requires_venue,
                    is_range,
                    comms_milestones,
                    default_logistics_slots,
                },
            )
            .collect(),
    }))
}

/// Timelines de com et postes logistiques par defaut : **de la donnee**, que
/// les reglages du collectif reecrivent sans toucher au code (§11.1).
#[derive(Deserialize)]
struct EventTypePatch {
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    requires_venue: Option<bool>,
    #[serde(default)]
    comms_milestones: Option<Vec<crate::services::comms::Milestone>>,
    #[serde(default)]
    default_logistics_slots: Option<serde_json::Value>,
}

async fn update_event_type(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, type_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<EventTypePatch>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let milestones = body
        .comms_milestones
        .map(|m| serde_json::to_value(m).unwrap_or(serde_json::Value::Null));

    let done = sqlx::query(
        "UPDATE event_types
            SET label = COALESCE($3, label),
                requires_venue = COALESCE($4, requires_venue),
                comms_milestones = COALESCE($5, comms_milestones),
                default_logistics_slots = COALESCE($6, default_logistics_slots)
          WHERE id = $1 AND collective_id = $2",
    )
    .bind(type_id)
    .bind(cid)
    .bind(body.label)
    .bind(body.requires_venue)
    .bind(milestones)
    .bind(body.default_logistics_slots)
    .execute(&state.db)
    .await?;

    if done.rows_affected() == 0 {
        return Err(AppError::not_found("type d'evenement introuvable"));
    }
    // Les evenements deja confirmes gardent leur plan : une affiche deja
    // programmee ne doit pas bouger parce qu'un jalon a change (§11.1).
    Ok(Json(json!({ "ok": true })))
}

#[derive(Serialize)]
struct MemberRow {
    user_id: Uuid,
    display_name: String,
    stage_name: Option<String>,
    /// Le telephone n'est visible que des admins du collectif (§3).
    phone: Option<String>,
    email: Option<String>,
    role: String,
    telegram_linked: bool,
    groups: Vec<Uuid>,
    pending_invitation: Option<String>,
}

async fn list_members(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<MemberRow>>> {
    let scope = state.scope(actor, cid).await?;
    let admin = scope.is_admin();

    let rows: Vec<(
        Uuid,
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        Option<i64>,
    )> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.stage_name, u.phone, u.email, m.role, u.telegram_id
             FROM memberships m JOIN users u ON u.id = m.user_id
             WHERE m.collective_id = $1 ORDER BY u.display_name",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let group_rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT gm.user_id, gm.group_id FROM group_members gm
         JOIN groups g ON g.id = gm.group_id WHERE g.collective_id = $1",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let invites: Vec<(Uuid, String)> = if admin {
        sqlx::query_as(
            "SELECT i.user_id, i.code FROM invitations i
             WHERE i.used_at IS NULL AND i.expires_at > now()",
        )
        .fetch_all(&state.db)
        .await?
    } else {
        vec![]
    };

    Ok(Json(
        rows.into_iter()
            .map(
                |(user_id, display_name, stage_name, phone, email, role, tg)| MemberRow {
                    user_id,
                    display_name,
                    stage_name,
                    phone: phone.filter(|_| admin),
                    email: email.filter(|_| admin),
                    role,
                    telegram_linked: tg.is_some(),
                    groups: group_rows
                        .iter()
                        .filter(|(u, _)| *u == user_id)
                        .map(|(_, g)| *g)
                        .collect(),
                    pending_invitation: invites
                        .iter()
                        .find(|(u, _)| *u == user_id)
                        .map(|(_, c)| c.clone()),
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewMember {
    display_name: String,
    #[serde(default)]
    stage_name: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    group_ids: Vec<Uuid>,
}

/// Un admin cree l'utilisateur et genere un **lien d'invitation a usage
/// unique** (§3). Un compte existant est simplement rattache : un utilisateur
/// est unique par personne et traverse les collectifs.
async fn create_member(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewMember>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let mut tx = state.db.begin().await?;

    let existing: Option<(Uuid,)> = match &body.email {
        Some(email) => {
            sqlx::query_as("SELECT id FROM users WHERE lower(email) = lower($1)")
                .bind(email)
                .fetch_optional(&mut *tx)
                .await?
        }
        None => None,
    };

    let user_id = match existing {
        Some((id,)) => id,
        None => {
            let (id,): (Uuid,) = sqlx::query_as(
                "INSERT INTO users (display_name, stage_name, phone, email)
                 VALUES ($1, $2, $3, $4) RETURNING id",
            )
            .bind(&body.display_name)
            .bind(&body.stage_name)
            .bind(&body.phone)
            .bind(&body.email)
            .fetch_one(&mut *tx)
            .await?;
            id
        }
    };

    let role = match body.role.as_deref() {
        Some("admin") => "admin",
        _ => "member",
    };
    sqlx::query(
        "INSERT INTO memberships (collective_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (collective_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(cid)
    .bind(user_id)
    .bind(role)
    .execute(&mut *tx)
    .await?;

    for gid in &body.group_ids {
        let ok: Option<(Uuid,)> =
            sqlx::query_as("SELECT id FROM groups WHERE id = $1 AND collective_id = $2")
                .bind(gid)
                .bind(cid)
                .fetch_optional(&mut *tx)
                .await?;
        if ok.is_none() {
            return Err(AppError::bad_request("groupe hors du collectif"));
        }
        sqlx::query(
            "INSERT INTO group_members (group_id, user_id) VALUES ($1, $2)
             ON CONFLICT DO NOTHING",
        )
        .bind(gid)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    }

    let code = crate::auth::session::random_token();
    sqlx::query(
        "INSERT INTO invitations (user_id, code, created_by, expires_at)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(&code)
    .bind(scope.user_id())
    .bind(Utc::now() + Duration::days(30))
    .execute(&mut *tx)
    .await?;

    crate::services::ical::ensure_token(&state.db, "user", user_id)
        .await
        .ok();
    tx.commit().await?;

    Ok(Json(json!({
        "user_id": user_id,
        "invitation_code": code,
        "invitation_url": format!("{}/invitation/{code}", state.config.public_base_url),
    })))
}

#[derive(Deserialize)]
struct UpdateMember {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    stage_name: Option<String>,
}

/// Le role d'admin est **un droit pose sur une appartenance**, attribuable a
/// n'importe qui et retirable (§3, §20).
async fn update_member(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, user_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateMember>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;

    if body.role.is_some() {
        scope.require_admin()?;
        let role = match body.role.as_deref() {
            Some("admin") => "admin",
            _ => "member",
        };
        // Ne jamais retirer le dernier admin : le collectif deviendrait
        // ingerable sans passer par l'admin d'instance.
        if role == "member" {
            let (admins,): (i64,) = sqlx::query_as(
                "SELECT count(*) FROM memberships WHERE collective_id = $1 AND role = 'admin'",
            )
            .bind(cid)
            .fetch_one(&state.db)
            .await?;
            let is_admin: Option<(String,)> = sqlx::query_as(
                "SELECT role FROM memberships WHERE collective_id = $1 AND user_id = $2",
            )
            .bind(cid)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
            if admins <= 1 && is_admin.map(|(r,)| r == "admin").unwrap_or(false) {
                return Err(AppError::conflict(
                    "c'est le dernier admin du collectif — en nommer un autre d'abord",
                ));
            }
        }
        sqlx::query("UPDATE memberships SET role = $3 WHERE collective_id = $1 AND user_id = $2")
            .bind(cid)
            .bind(user_id)
            .bind(role)
            .execute(&state.db)
            .await?;
    }

    // Chacun modifie ses propres coordonnees ; un admin peut aussi le faire.
    if body.phone.is_some() || body.stage_name.is_some() {
        if scope.user_id() != user_id {
            scope.require_admin()?;
        }
        sqlx::query(
            "UPDATE users SET phone = COALESCE($2, phone), stage_name = COALESCE($3, stage_name),
                              updated_at = now() WHERE id = $1",
        )
        .bind(user_id)
        .bind(&body.phone)
        .bind(&body.stage_name)
        .execute(&state.db)
        .await?;
    }

    Ok(Json(json!({ "ok": true })))
}

async fn regenerate_invitation(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, user_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let member: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM memberships WHERE collective_id = $1 AND user_id = $2")
            .bind(cid)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    member.ok_or_else(|| AppError::not_found("membre introuvable"))?;

    sqlx::query("UPDATE invitations SET expires_at = now() WHERE user_id = $1 AND used_at IS NULL")
        .bind(user_id)
        .execute(&state.db)
        .await?;

    let code = crate::auth::session::random_token();
    sqlx::query(
        "INSERT INTO invitations (user_id, code, created_by, expires_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(&code)
    .bind(scope.user_id())
    .bind(Utc::now() + Duration::days(30))
    .execute(&state.db)
    .await?;

    Ok(Json(json!({
        "invitation_code": code,
        "invitation_url": format!("{}/invitation/{code}", state.config.public_base_url),
    })))
}

#[derive(Serialize)]
struct GroupRow {
    id: Uuid,
    slug: String,
    name: String,
    description: Option<String>,
    members: Vec<GroupMemberRow>,
    has_tech_rider: bool,
}

#[derive(Serialize)]
struct GroupMemberRow {
    user_id: Uuid,
    display_name: String,
    stage_name: Option<String>,
    role_label: Option<String>,
    is_admin: bool,
}

async fn list_groups(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<GroupRow>>> {
    let _scope = state.scope(actor, cid).await?;
    let groups: Vec<(Uuid, String, String, Option<String>)> = sqlx::query_as(
        "SELECT id, slug, name, description FROM groups WHERE collective_id = $1 ORDER BY name",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let mut out = Vec::new();
    for (id, slug, name, description) in groups {
        out.push(GroupRow {
            id,
            slug,
            name,
            description,
            members: group_members(&state, id).await?,
            has_tech_rider: sqlx::query_as::<_, (i64,)>(
                "SELECT count(*) FROM tech_riders WHERE group_id = $1 AND status = 'published'",
            )
            .bind(id)
            .fetch_one(&state.db)
            .await?
            .0 > 0,
        });
    }
    Ok(Json(out))
}

async fn group_members(state: &AppState, group_id: Uuid) -> AppResult<Vec<GroupMemberRow>> {
    let rows: Vec<(Uuid, String, Option<String>, Option<String>, bool)> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.stage_name, gm.role_label, gm.is_admin
         FROM group_members gm JOIN users u ON u.id = gm.user_id
         WHERE gm.group_id = $1 ORDER BY u.display_name",
    )
    .bind(group_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(user_id, display_name, stage_name, role_label, is_admin)| GroupMemberRow {
                user_id,
                display_name,
                stage_name,
                role_label,
                is_admin,
            },
        )
        .collect())
}

#[derive(Deserialize)]
struct NewGroup {
    slug: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
}

async fn create_group(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewGroup>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO groups (collective_id, slug, name, description) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(cid)
    .bind(&body.slug)
    .bind(&body.name)
    .bind(&body.description)
    .fetch_one(&state.db)
    .await?;

    // Charte locale : surcharge partielle de celle du collectif (§8).
    provisioning::seed_brand(&state.db, cid, &body.name, Some(id)).await?;
    crate::services::ical::ensure_token(&state.db, "group", id).await?;

    Ok(Json(json!({ "id": id })))
}

async fn show_group(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<GroupRow>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    let (slug, name, description): (String, String, Option<String>) =
        sqlx::query_as("SELECT slug, name, description FROM groups WHERE id = $1")
            .bind(gid)
            .fetch_one(&state.db)
            .await?;
    Ok(Json(GroupRow {
        id: gid,
        slug,
        name,
        description,
        members: group_members(&state, gid).await?,
        has_tech_rider: sqlx::query_as::<_, (i64,)>(
            "SELECT count(*) FROM tech_riders WHERE group_id = $1 AND status = 'published'",
        )
        .bind(gid)
        .fetch_one(&state.db)
        .await?
        .0 > 0,
    }))
}

#[derive(Deserialize)]
struct UpdateGroup {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

async fn update_group(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateGroup>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;
    sqlx::query(
        "UPDATE groups SET name = COALESCE($2, name), description = COALESCE($3, description)
         WHERE id = $1",
    )
    .bind(gid)
    .bind(&body.name)
    .bind(&body.description)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct NewGroupMember {
    user_id: Uuid,
    /// Role libre : « MAO », « batterie »… jamais un catalogue (§4).
    #[serde(default)]
    role_label: Option<String>,
    #[serde(default)]
    is_admin: bool,
}

async fn add_group_member(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewGroupMember>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;

    // Le groupe appartient au collectif ; la personne doit y etre membre.
    let member: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM memberships WHERE collective_id = $1 AND user_id = $2")
            .bind(cid)
            .bind(body.user_id)
            .fetch_optional(&state.db)
            .await?;
    member.ok_or_else(|| AppError::bad_request("cette personne n'est pas membre du collectif"))?;

    sqlx::query(
        "INSERT INTO group_members (group_id, user_id, role_label, is_admin)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (group_id, user_id)
         DO UPDATE SET role_label = EXCLUDED.role_label, is_admin = EXCLUDED.is_admin",
    )
    .bind(gid)
    .bind(body.user_id)
    .bind(&body.role_label)
    .bind(body.is_admin)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "ok": true })))
}

async fn remove_group_member(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, gid, user_id)): Path<(Uuid, Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.check_group(&state.db, gid).await?;
    scope.require_group_admin(&state.db, gid).await?;
    sqlx::query("DELETE FROM group_members WHERE group_id = $1 AND user_id = $2")
        .bind(gid)
        .bind(user_id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
