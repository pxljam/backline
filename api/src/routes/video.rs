//! Video declarative et rendu distribue (§10).
//!
//! Backline **decrit** la video ; une CLI la fabrique sur la machine d'un
//! membre. Le VPS n'encode jamais.

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{jobs, notify};
use crate::state::AppState;
use axum::extract::{Multipart, Path, State};
use axum::http::request::Parts;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

/// Routes propres a un collectif.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/video-compositions",
            get(list_compositions).post(create_composition),
        )
        .route(
            "/video-compositions/:composition_id",
            get(show_composition).put(save_composition),
        )
        .route(
            "/video-compositions/:composition_id/render",
            post(queue_render),
        )
        .route("/render-jobs", get(list_jobs))
}

/// Routes de la CLI et des machines, hors perimetre collectif.
pub fn machine_router() -> Router<AppState> {
    Router::new()
        .route("/machines", get(list_machines).post(register_machine))
        .route(
            "/machines/:machine_id",
            axum::routing::delete(revoke_machine),
        )
        .route("/claim", post(claim_job))
        .route("/jobs/:job_id/bundle", get(job_bundle))
        .route("/jobs/:job_id/progress", put(report_progress))
        .route("/jobs/:job_id/complete", post(complete_job))
        .route("/jobs/:job_id/fail", post(fail_job))
}

// --- Compositions ---------------------------------------------------------

#[derive(Serialize)]
struct CompositionRow {
    id: Uuid,
    name: String,
    group_id: Option<Uuid>,
    event_id: Option<Uuid>,
    format_id: Uuid,
    fps: i32,
    version: i32,
    spec: Value,
    updated_at: DateTime<Utc>,
}

async fn list_compositions(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<CompositionRow>>> {
    let _scope = state.scope(actor, cid).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        Uuid,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Uuid,
        i32,
        i32,
        Value,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, name, group_id, event_id, format_id, fps, version, spec, updated_at
             FROM video_compositions WHERE collective_id = $1 ORDER BY updated_at DESC",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, name, group_id, event_id, format_id, fps, version, spec, updated_at)| {
                    CompositionRow {
                        id,
                        name,
                        group_id,
                        event_id,
                        format_id,
                        fps,
                        version,
                        spec,
                        updated_at,
                    }
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewComposition {
    name: String,
    format_id: Uuid,
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    event_id: Option<Uuid>,
    #[serde(default = "thirty")]
    fps: i32,
    #[serde(default)]
    spec: Option<Value>,
}

fn thirty() -> i32 {
    30
}

async fn create_composition(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewComposition>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    if let Some(gid) = body.group_id {
        scope.check_group(&state.db, gid).await?;
    }
    if let Some(eid) = body.event_id {
        crate::routes::events::check_event(&state, cid, eid).await?;
    }

    let size: Option<(i32, i32)> = sqlx::query_as(
        "SELECT width, height FROM formats WHERE id = $1 AND (collective_id IS NULL OR collective_id = $2)",
    )
    .bind(body.format_id)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (w, h) = size.ok_or_else(|| AppError::not_found("format introuvable"))?;

    // La composition est une description : des plans, des calques, une piste
    // audio. Le fichier n'existe pas encore, et c'est le principe (§10.2).
    let spec = body.spec.unwrap_or_else(|| {
        json!({
            "version": 1,
            "width": w,
            "height": h,
            "fps": body.fps,
            "scenes": [],
            "audio": null
        })
    });

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO video_compositions
            (collective_id, group_id, event_id, name, format_id, fps, spec, created_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
    )
    .bind(cid)
    .bind(body.group_id)
    .bind(body.event_id)
    .bind(&body.name)
    .bind(body.format_id)
    .bind(body.fps)
    .bind(&spec)
    .bind(scope.user_id())
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({ "id": id })))
}

async fn show_composition(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, vid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<CompositionRow>> {
    let _scope = state.scope(actor, cid).await?;
    #[allow(clippy::type_complexity)]
    let row: Option<(
        Uuid,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Uuid,
        i32,
        i32,
        Value,
        DateTime<Utc>,
    )> = sqlx::query_as(
        "SELECT id, name, group_id, event_id, format_id, fps, version, spec, updated_at
             FROM video_compositions WHERE id = $1 AND collective_id = $2",
    )
    .bind(vid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (id, name, group_id, event_id, format_id, fps, version, spec, updated_at) =
        row.ok_or_else(|| AppError::not_found("composition introuvable"))?;
    Ok(Json(CompositionRow {
        id,
        name,
        group_id,
        event_id,
        format_id,
        fps,
        version,
        spec,
        updated_at,
    }))
}

#[derive(Deserialize)]
struct SaveComposition {
    spec: Value,
    #[serde(default)]
    name: Option<String>,
}

async fn save_composition(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, vid)): Path<(Uuid, Uuid)>,
    Json(body): Json<SaveComposition>,
) -> AppResult<Json<serde_json::Value>> {
    let _scope = state.scope(actor, cid).await?;
    let (version,): (i32,) = sqlx::query_as(
        "UPDATE video_compositions SET spec = $3, name = COALESCE($4, name),
                                       version = version + 1, updated_at = now()
         WHERE id = $1 AND collective_id = $2 RETURNING version",
    )
    .bind(vid)
    .bind(cid)
    .bind(&body.spec)
    .bind(&body.name)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::not_found("composition introuvable"))?;
    Ok(Json(json!({ "version": version })))
}

#[derive(Deserialize)]
struct QueueRender {
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    publication_task_id: Option<Uuid>,
}

async fn queue_render(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, vid)): Path<(Uuid, Uuid)>,
    Json(body): Json<QueueRender>,
) -> AppResult<Json<serde_json::Value>> {
    let _scope = state.scope(actor, cid).await?;
    let comp: Option<(Uuid, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, event_id FROM video_compositions WHERE id = $1 AND collective_id = $2",
    )
    .bind(vid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (_, event_id) = comp.ok_or_else(|| AppError::not_found("composition introuvable"))?;

    let kind = match body.kind.as_deref() {
        Some("preview") => "preview",
        _ => "video",
    };

    let (job_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO render_jobs (collective_id, composition_id, event_id, publication_task_id, kind)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(cid)
    .bind(vid)
    .bind(event_id)
    .bind(body.publication_task_id)
    .bind(kind)
    .fetch_one(&state.db)
    .await?;

    // Un job non reclame au bout d'un delai **alerte l'admin** plutot que de
    // rester silencieusement en attente (§10.3).
    jobs::enqueue(
        &state.db,
        "render_unclaimed_alert",
        Utc::now() + Duration::minutes(30),
        json!({ "render_job_id": job_id }),
        Some(format!("render:{job_id}:unclaimed")),
    )
    .await?;

    let (machines,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM render_machines
         WHERE revoked_at IS NULL AND last_seen_at > now() - interval '10 minutes'",
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(json!({ "id": job_id, "machines_online": machines })))
}

#[derive(Serialize)]
struct JobRow {
    id: Uuid,
    composition_id: Option<Uuid>,
    composition_name: Option<String>,
    kind: String,
    status: String,
    progress: f32,
    error: Option<String>,
    output_asset_id: Option<Uuid>,
    claimed_by: Option<String>,
    created_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
}

/// Ecran « Rendus » : file des jobs, machines connectees, progression, erreurs
/// (§14). L'application affiche **en permanence** quelles machines sont la.
async fn list_jobs(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let _scope = state.scope(actor, cid).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        Uuid,
        Option<Uuid>,
        Option<String>,
        String,
        String,
        f32,
        Option<String>,
        Option<Uuid>,
        Option<String>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT r.id, r.composition_id, c.name, r.kind, r.status, r.progress, r.error,
                    r.output_asset_id, m.name, r.created_at, r.finished_at
             FROM render_jobs r
             LEFT JOIN video_compositions c ON c.id = r.composition_id
             LEFT JOIN render_machines m ON m.id = r.claimed_by
             WHERE r.collective_id = $1 ORDER BY r.created_at DESC LIMIT 100",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;

    let machines: Vec<(Uuid, String, Value, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT id, name, capabilities, last_seen_at FROM render_machines
         WHERE revoked_at IS NULL ORDER BY last_seen_at DESC NULLS LAST",
    )
    .fetch_all(&state.db)
    .await?;

    let jobs: Vec<JobRow> = rows
        .into_iter()
        .map(
            |(
                id,
                composition_id,
                composition_name,
                kind,
                status,
                progress,
                error,
                output_asset_id,
                claimed_by,
                created_at,
                finished_at,
            )| {
                JobRow {
                    id,
                    composition_id,
                    composition_name,
                    kind,
                    status,
                    progress,
                    error,
                    output_asset_id,
                    claimed_by,
                    created_at,
                    finished_at,
                }
            },
        )
        .collect();

    Ok(Json(json!({
        "jobs": jobs,
        "machines": machines.into_iter().map(|(id, name, capabilities, last_seen_at)| json!({
            "id": id, "name": name, "capabilities": capabilities, "last_seen_at": last_seen_at,
            "online": last_seen_at.is_some_and(|t| Utc::now() - t < Duration::minutes(10)),
        })).collect::<Vec<_>>(),
    })))
}

// --- Machines de rendu ----------------------------------------------------

#[derive(Serialize)]
struct MachineRow {
    id: Uuid,
    name: String,
    capabilities: Value,
    last_seen_at: Option<DateTime<Utc>>,
    revoked: bool,
    mine: bool,
}

async fn list_machines(
    State(state): State<AppState>,
    Auth(actor): Auth,
) -> AppResult<Json<Vec<MachineRow>>> {
    let rows: Vec<(
        Uuid,
        Uuid,
        String,
        Value,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        "SELECT id, user_id, name, capabilities, last_seen_at, revoked_at
             FROM render_machines ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, user_id, name, capabilities, last_seen_at, revoked_at)| MachineRow {
                    id,
                    name,
                    capabilities,
                    last_seen_at,
                    revoked: revoked_at.is_some(),
                    mine: user_id == actor.user_id,
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewMachine {
    name: String,
    #[serde(default)]
    capabilities: Option<Value>,
}

/// `backline login` : associe la machine au compte, par un jeton **revocable**
/// (§10.3). Le jeton n'est montre qu'une fois.
async fn register_machine(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Json(body): Json<NewMachine>,
) -> AppResult<Json<serde_json::Value>> {
    let token = crate::auth::session::random_token();
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO render_machines (user_id, name, token_hash, capabilities)
         VALUES ($1, $2, $3, COALESCE($4, '{}'::jsonb)) RETURNING id",
    )
    .bind(actor.user_id)
    .bind(&body.name)
    .bind(crate::auth::session::hash_token(&token))
    .bind(&body.capabilities)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({ "id": id, "token": token })))
}

async fn revoke_machine(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(mid): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let owner: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM render_machines WHERE id = $1")
            .bind(mid)
            .fetch_optional(&state.db)
            .await?;
    let (owner,) = owner.ok_or_else(|| AppError::not_found("machine introuvable"))?;
    if owner != actor.user_id && !actor.is_instance_admin {
        return Err(AppError::forbidden("cette machine n'est pas la tienne"));
    }
    sqlx::query("UPDATE render_machines SET revoked_at = now() WHERE id = $1")
        .bind(mid)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// Authentification d'une machine de rendu, par en-tete `X-Machine-Token`.
pub struct MachineAuth {
    pub machine_id: Uuid,
}

#[async_trait::async_trait]
impl axum::extract::FromRequestParts<AppState> for MachineAuth {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("x-machine-token")
            .and_then(|v| v.to_str().ok())
            .ok_or(AppError::Unauthorized)?;

        let row: Option<(Uuid,)> = sqlx::query_as(
            "SELECT id FROM render_machines WHERE token_hash = $1 AND revoked_at IS NULL",
        )
        .bind(crate::auth::session::hash_token(token))
        .fetch_optional(&state.db)
        .await?;
        let (machine_id,) = row.ok_or(AppError::Unauthorized)?;

        // Chaque appel vaut signe de vie : c'est ce qui alimente « machines
        // connectees » dans l'interface.
        sqlx::query("UPDATE render_machines SET last_seen_at = now() WHERE id = $1")
            .bind(machine_id)
            .execute(&state.db)
            .await?;

        Ok(MachineAuth { machine_id })
    }
}

#[derive(Deserialize)]
struct ClaimRequest {
    #[serde(default)]
    capabilities: Option<Value>,
}

/// `backline render` : la machine **reclame** un job. Meme mecanisme que le
/// reste de la file — `FOR UPDATE SKIP LOCKED`, une seule file a comprendre.
async fn claim_job(
    State(state): State<AppState>,
    machine: MachineAuth,
    Json(body): Json<ClaimRequest>,
) -> AppResult<Json<Value>> {
    if let Some(caps) = &body.capabilities {
        sqlx::query("UPDATE render_machines SET capabilities = $2 WHERE id = $1")
            .bind(machine.machine_id)
            .bind(caps)
            .execute(&state.db)
            .await?;
    }

    let claimed: Option<(Uuid, Option<Uuid>, String)> = sqlx::query_as(
        "UPDATE render_jobs SET status = 'claimed', claimed_by = $1, claimed_at = now(),
                                attempts = attempts + 1
         WHERE id = (
            SELECT id FROM render_jobs WHERE status = 'queued'
            ORDER BY created_at FOR UPDATE SKIP LOCKED LIMIT 1)
         RETURNING id, composition_id, kind",
    )
    .bind(machine.machine_id)
    .fetch_optional(&state.db)
    .await?;

    match claimed {
        Some((id, composition_id, kind)) => Ok(Json(
            json!({ "job": { "id": id, "composition_id": composition_id, "kind": kind } }),
        )),
        None => Ok(Json(json!({ "job": null }))),
    }
}

/// Tout ce qu'il faut pour rendre hors ligne : la description, la charte, les
/// champs automatiques, et des **URL signees** vers les medias du job (§10.3).
async fn job_bundle(
    State(state): State<AppState>,
    machine: MachineAuth,
    Path(job_id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let row: Option<(Uuid, Option<Uuid>, Option<Uuid>, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT collective_id, composition_id, event_id, kind, claimed_by
         FROM render_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?;
    let (collective_id, composition_id, event_id, kind, claimed_by) =
        row.ok_or_else(|| AppError::not_found("job introuvable"))?;

    // Une machine ne lit que ce qu'elle a reclame : acces limite aux medias du
    // job (§10.3, traitement du risque).
    if claimed_by != Some(machine.machine_id) {
        return Err(AppError::forbidden(
            "ce job est reclame par une autre machine",
        ));
    }

    let comp: Option<(String, i32, Value, Option<Uuid>)> = match composition_id {
        Some(id) => {
            sqlx::query_as("SELECT name, fps, spec, group_id FROM video_compositions WHERE id = $1")
                .bind(id)
                .fetch_optional(&state.db)
                .await?
        }
        None => None,
    };
    let (name, fps, spec, group_id) =
        comp.ok_or_else(|| AppError::not_found("composition introuvable"))?;

    let brand = crate::services::visuals::resolve_brand(&state.db, collective_id, group_id).await?;
    let data = match event_id {
        Some(eid) => crate::services::templates::resolve_fields(&state.db, eid).await?,
        None => json!({}),
    };

    // Les assets references par la description recoivent une URL signee.
    let mut media = serde_json::Map::new();
    for asset_id in collect_asset_ids(&spec) {
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT storage_key, mime FROM assets WHERE id = $1 AND collective_id = $2",
        )
        .bind(asset_id)
        .bind(collective_id)
        .fetch_optional(&state.db)
        .await?;
        if let Some((key, mime)) = row {
            let url = state
                .storage
                .signed_url(&key, 6 * 3600)
                .await
                .map_err(AppError::Internal)?;
            media.insert(asset_id.to_string(), json!({ "url": url, "mime": mime }));
        }
    }

    sqlx::query("UPDATE render_jobs SET status = 'running' WHERE id = $1")
        .bind(job_id)
        .execute(&state.db)
        .await?;

    Ok(Json(json!({
        "id": job_id,
        "kind": kind,
        "name": name,
        "fps": fps,
        "spec": spec,
        "brand": brand,
        "data": data,
        "media": media,
    })))
}

/// Cherche les `assetId` partout dans la description, sans connaitre sa forme
/// exacte : la description evolue, la collecte n'a pas a suivre.
fn collect_asset_ids(spec: &Value) -> Vec<Uuid> {
    let mut out = Vec::new();
    walk(spec, &mut out);
    out.sort();
    out.dedup();
    out
}

fn walk(v: &Value, out: &mut Vec<Uuid>) {
    match v {
        Value::Object(map) => {
            for (k, val) in map {
                if (k == "assetId" || k == "asset_id") && val.is_string() {
                    if let Ok(id) = Uuid::parse_str(val.as_str().unwrap()) {
                        out.push(id);
                    }
                }
                walk(val, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
        _ => {}
    }
}

#[derive(Deserialize)]
struct Progress {
    progress: f32,
}

async fn report_progress(
    State(state): State<AppState>,
    machine: MachineAuth,
    Path(job_id): Path<Uuid>,
    Json(body): Json<Progress>,
) -> AppResult<Json<Value>> {
    sqlx::query(
        "UPDATE render_jobs SET progress = LEAST(GREATEST($3, 0), 1), status = 'running'
         WHERE id = $1 AND claimed_by = $2",
    )
    .bind(job_id)
    .bind(machine.machine_id)
    .bind(body.progress)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

/// La machine renvoie le fichier fini ; il devient un asset du collectif.
async fn complete_job(
    State(state): State<AppState>,
    machine: MachineAuth,
    Path(job_id): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let row: Option<(Uuid, Option<Uuid>, Option<Uuid>)> = sqlx::query_as(
        "SELECT collective_id, composition_id, claimed_by FROM render_jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(&state.db)
    .await?;
    let (collective_id, composition_id, claimed_by) =
        row.ok_or_else(|| AppError::not_found("job introuvable"))?;
    if claimed_by != Some(machine.machine_id) {
        return Err(AppError::forbidden(
            "ce job est reclame par une autre machine",
        ));
    }

    let mut data = Vec::new();
    let mut mime = "video/mp4".to_string();
    let mut filename = format!("{job_id}.mp4");
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("formulaire illisible : {e}")))?
    {
        if field.name() == Some("file") {
            filename = field.file_name().unwrap_or(&filename).to_string();
            mime = field.content_type().unwrap_or("video/mp4").to_string();
            data = field
                .bytes()
                .await
                .map_err(|e| AppError::bad_request(format!("lecture du fichier : {e}")))?
                .to_vec();
        }
    }
    if data.is_empty() {
        return Err(AppError::bad_request("aucun fichier recu"));
    }

    let asset_id = Uuid::new_v4();
    let key = format!("collectives/{collective_id}/render/{asset_id}.mp4");
    state
        .storage
        .put(&key, data.clone(), &mime)
        .await
        .map_err(AppError::Internal)?;

    let group_id: Option<Uuid> = match composition_id {
        Some(c) => sqlx::query_as::<_, (Option<Uuid>,)>(
            "SELECT group_id FROM video_compositions WHERE id = $1",
        )
        .bind(c)
        .fetch_optional(&state.db)
        .await?
        .and_then(|(g,)| g),
        None => None,
    };

    sqlx::query(
        "INSERT INTO assets (id, collective_id, group_id, kind, filename, storage_key, mime, bytes, purge_after)
         VALUES ($1, $2, $3, 'render', $4, $5, $6, $7, $8)",
    )
    .bind(asset_id)
    .bind(collective_id)
    .bind(group_id)
    .bind(&filename)
    .bind(&key)
    .bind(&mime)
    .bind(data.len() as i64)
    .bind(Utc::now() + Duration::days(183))
    .execute(&state.db)
    .await?;

    sqlx::query(
        "UPDATE render_jobs SET status = 'done', progress = 1, output_asset_id = $2,
                                finished_at = now(), error = NULL
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(asset_id)
    .execute(&state.db)
    .await?;

    jobs::cancel_by_prefix(&state.db, &format!("render:{job_id}:")).await?;

    Ok(Json(json!({ "asset_id": asset_id })))
}

#[derive(Deserialize)]
struct FailJob {
    error: String,
}

async fn fail_job(
    State(state): State<AppState>,
    machine: MachineAuth,
    Path(job_id): Path<Uuid>,
    Json(body): Json<FailJob>,
) -> AppResult<Json<Value>> {
    // Le job retourne dans la file : une autre machine peut reprendre. Au-dela
    // de trois tentatives, on arrete et on previent.
    let row: Option<(i32, Uuid)> = sqlx::query_as(
        "UPDATE render_jobs
         SET status = CASE WHEN attempts >= 3 THEN 'failed' ELSE 'queued' END,
             error = $3, claimed_by = NULL,
             finished_at = CASE WHEN attempts >= 3 THEN now() ELSE NULL END
         WHERE id = $1 AND claimed_by = $2
         RETURNING attempts, collective_id",
    )
    .bind(job_id)
    .bind(machine.machine_id)
    .bind(&body.error)
    .fetch_optional(&state.db)
    .await?;

    if let Some((attempts, collective_id)) = row {
        if attempts >= 3 {
            for user_id in notify::collective_admin_ids(&state.db, collective_id).await? {
                notify::push(
                    &state.db,
                    notify::Notice {
                        user_id,
                        collective_id: Some(collective_id),
                        kind: "admin_alert",
                        title: "Rendu video en echec".into(),
                        body: format!(
                            "{} — la tache de com reste livrable avec son visuel fixe.",
                            body.error
                        ),
                        payload: json!({ "render_job_id": job_id }),
                    },
                )
                .await?;
            }
        }
    }

    Ok(Json(json!({ "ok": true })))
}
