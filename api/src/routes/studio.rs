//! Studio : charte, catalogue de formats, gabarits, bibliotheque de medias (§8, §9).

use crate::error::{AppError, AppResult};
use crate::extract::Auth;
use crate::services::{formats as fmt, templates};
use crate::state::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/brand", get(show_brand))
        .route("/brand/tokens", put(save_tokens))
        .route("/formats", get(list_formats).post(create_format))
        .route("/templates", get(list_templates).post(create_template))
        .route(
            "/templates/:template_id",
            get(show_template).patch(update_template),
        )
        .route("/templates/:template_id/variants", post(add_variant))
        .route(
            "/templates/:template_id/variants/:format_id",
            put(save_variant),
        )
        .route(
            "/templates/:template_id/duplicate",
            post(duplicate_template),
        )
        .route("/assets", get(list_assets).post(upload_asset))
        .route("/assets/:asset_id/url", get(asset_url))
        .route("/preview-fields/:event_id", get(preview_fields))
}

// --- Charte ---------------------------------------------------------------

#[derive(Serialize)]
struct BrandView {
    id: Uuid,
    name: String,
    group_id: Option<Uuid>,
    tokens: Vec<TokenRow>,
    /// Charte du collectif, quand on regarde celle d'un groupe : la surcharge
    /// est **partielle**, le reste est herite (§8).
    inherited: Option<Vec<TokenRow>>,
}

#[derive(Serialize, Deserialize, Clone)]
struct TokenRow {
    kind: String,
    key: String,
    label: String,
    value: Value,
    #[serde(default)]
    position: i32,
}

#[derive(Deserialize)]
struct BrandQuery {
    #[serde(default)]
    group_id: Option<Uuid>,
}

async fn show_brand(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Query(q): Query<BrandQuery>,
) -> AppResult<Json<BrandView>> {
    let scope = state.scope(actor, cid).await?;
    if let Some(gid) = q.group_id {
        scope.check_group(&state.db, gid).await?;
    }

    let brand: Option<(Uuid, String)> = match q.group_id {
        Some(gid) => {
            sqlx::query_as("SELECT id, name FROM brands WHERE group_id = $1")
                .bind(gid)
                .fetch_optional(&state.db)
                .await?
        }
        None => {
            sqlx::query_as(
                "SELECT id, name FROM brands WHERE collective_id = $1 AND group_id IS NULL",
            )
            .bind(cid)
            .fetch_optional(&state.db)
            .await?
        }
    };
    let (brand_id, name) = brand.ok_or_else(|| AppError::not_found("charte introuvable"))?;

    let inherited = match q.group_id {
        Some(_) => {
            let parent: Option<(Uuid,)> = sqlx::query_as(
                "SELECT id FROM brands WHERE collective_id = $1 AND group_id IS NULL",
            )
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
            match parent {
                Some((pid,)) => Some(tokens_of(&state, pid).await?),
                None => None,
            }
        }
        None => None,
    };

    Ok(Json(BrandView {
        id: brand_id,
        name,
        group_id: q.group_id,
        tokens: tokens_of(&state, brand_id).await?,
        inherited,
    }))
}

async fn tokens_of(state: &AppState, brand_id: Uuid) -> AppResult<Vec<TokenRow>> {
    let rows: Vec<(String, String, String, Value, i32)> = sqlx::query_as(
        "SELECT kind, key, label, value, position FROM brand_tokens
         WHERE brand_id = $1 ORDER BY kind, position",
    )
    .bind(brand_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(kind, key, label, value, position)| TokenRow {
            kind,
            key,
            label,
            value,
            position,
        })
        .collect())
}

#[derive(Deserialize)]
struct SaveTokens {
    #[serde(default)]
    group_id: Option<Uuid>,
    tokens: Vec<TokenRow>,
}

/// La charte est **de la donnee**, jamais du code : saisir les vraies valeurs
/// de Bonsoir Techno ne demande aucune modification de code (§8).
async fn save_tokens(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<SaveTokens>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    let brand: Option<(Uuid,)> = match body.group_id {
        Some(gid) => {
            scope.check_group(&state.db, gid).await?;
            scope.require_group_admin(&state.db, gid).await?;
            sqlx::query_as("SELECT id FROM brands WHERE group_id = $1")
                .bind(gid)
                .fetch_optional(&state.db)
                .await?
        }
        None => {
            scope.require_admin()?;
            sqlx::query_as("SELECT id FROM brands WHERE collective_id = $1 AND group_id IS NULL")
                .bind(cid)
                .fetch_optional(&state.db)
                .await?
        }
    };
    let (brand_id,) = brand.ok_or_else(|| AppError::not_found("charte introuvable"))?;

    let mut tx = state.db.begin().await?;
    for t in &body.tokens {
        if !matches!(t.kind.as_str(), "color" | "font" | "logo" | "grid" | "rule") {
            return Err(AppError::bad_request(format!(
                "type de token inconnu : {}",
                t.kind
            )));
        }
        sqlx::query(
            "INSERT INTO brand_tokens (brand_id, kind, key, label, value, position)
             VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (brand_id, kind, key)
             DO UPDATE SET label = EXCLUDED.label, value = EXCLUDED.value, position = EXCLUDED.position",
        )
        .bind(brand_id)
        .bind(&t.kind)
        .bind(&t.key)
        .bind(&t.label)
        .bind(&t.value)
        .bind(t.position)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query("UPDATE brands SET updated_at = now() WHERE id = $1")
        .bind(brand_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

// --- Formats --------------------------------------------------------------

#[derive(Serialize)]
struct FormatRow {
    id: Uuid,
    key: String,
    platform: String,
    label: String,
    width: i32,
    height: i32,
    kind: String,
    ratio: String,
    /// `true` pour les formats propres au collectif, ajoutes a la main.
    custom: bool,
}

async fn list_formats(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<FormatRow>>> {
    let _scope = state.scope(actor, cid).await?;
    let rows: Vec<(Uuid, String, String, String, i32, i32, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT id, key, platform, label, width, height, kind, collective_id FROM formats
         WHERE (collective_id IS NULL OR collective_id = $1) AND archived = FALSE
         ORDER BY position, label",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(
                |(id, key, platform, label, width, height, kind, owner)| FormatRow {
                    id,
                    key,
                    platform,
                    label,
                    width,
                    height,
                    kind,
                    ratio: fmt::ratio_label(width, height),
                    custom: owner.is_some(),
                },
            )
            .collect(),
    ))
}

#[derive(Deserialize)]
struct NewFormat {
    key: String,
    platform: String,
    label: String,
    width: i32,
    height: i32,
    #[serde(default)]
    kind: Option<String>,
}

/// Le catalogue est editable : les plateformes changent leurs formats, cela ne
/// doit jamais demander une modification de code (§9.2).
async fn create_format(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewFormat>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    if body.width <= 0 || body.height <= 0 {
        return Err(AppError::bad_request("dimensions invalides"));
    }
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO formats (collective_id, key, platform, label, width, height, kind, position)
         VALUES ($1, $2, $3, $4, $5, $6, COALESCE($7, 'image'), 100) RETURNING id",
    )
    .bind(cid)
    .bind(&body.key)
    .bind(&body.platform)
    .bind(&body.label)
    .bind(body.width)
    .bind(body.height)
    .bind(&body.kind)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(
        json!({ "id": id, "ratio": fmt::ratio_label(body.width, body.height) }),
    ))
}

// --- Gabarits -------------------------------------------------------------

#[derive(Serialize)]
struct TemplateRow {
    id: Uuid,
    name: String,
    group_id: Option<Uuid>,
    master_format_id: Uuid,
    milestone_key: Option<String>,
    version: i32,
    variants: Vec<VariantRow>,
}

#[derive(Serialize)]
struct VariantRow {
    format_id: Uuid,
    format_key: String,
    width: i32,
    height: i32,
    ratio: String,
    is_master: bool,
    layout: Value,
}

async fn list_templates(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<TemplateRow>>> {
    let _scope = state.scope(actor, cid).await?;
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM templates WHERE collective_id = $1 AND archived = FALSE ORDER BY name",
    )
    .bind(cid)
    .fetch_all(&state.db)
    .await?;
    let mut out = Vec::new();
    for (id,) in rows {
        out.push(load_template(&state, cid, id).await?);
    }
    Ok(Json(out))
}

async fn load_template(state: &AppState, cid: Uuid, tid: Uuid) -> AppResult<TemplateRow> {
    let row: Option<(Uuid, String, Option<Uuid>, Uuid, Option<String>, i32)> = sqlx::query_as(
        "SELECT id, name, group_id, master_format_id, milestone_key, version FROM templates
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(tid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (id, name, group_id, master_format_id, milestone_key, version) =
        row.ok_or_else(|| AppError::not_found("gabarit introuvable"))?;

    let variants: Vec<(Uuid, String, i32, i32, bool, Value)> = sqlx::query_as(
        "SELECT v.format_id, f.key, f.width, f.height, v.is_master, v.layout
         FROM template_variants v JOIN formats f ON f.id = v.format_id
         WHERE v.template_id = $1 ORDER BY v.is_master DESC, f.position",
    )
    .bind(tid)
    .fetch_all(&state.db)
    .await?;

    Ok(TemplateRow {
        id,
        name,
        group_id,
        master_format_id,
        milestone_key,
        version,
        variants: variants
            .into_iter()
            .map(
                |(format_id, format_key, width, height, is_master, layout)| VariantRow {
                    format_id,
                    format_key,
                    width,
                    height,
                    ratio: fmt::ratio_label(width, height),
                    is_master,
                    layout,
                },
            )
            .collect(),
    })
}

async fn show_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<TemplateRow>> {
    let _scope = state.scope(actor, cid).await?;
    Ok(Json(load_template(&state, cid, tid).await?))
}

#[derive(Deserialize)]
struct NewTemplate {
    name: String,
    master_format_id: Uuid,
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    milestone_key: Option<String>,
    #[serde(default)]
    layout: Option<Value>,
}

async fn create_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewTemplate>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    if let Some(gid) = body.group_id {
        scope.check_group(&state.db, gid).await?;
        scope.require_group_admin(&state.db, gid).await?;
    } else {
        scope.require_admin()?;
    }

    let (w, h) = format_size(&state, cid, body.master_format_id).await?;
    let layout = match body.layout {
        Some(mut l) => {
            // Le cadre ne peut pas quitter le ratio du format (§9.2, §18) :
            // on impose les dimensions, on ne les negocie pas.
            l["width"] = json!(w);
            l["height"] = json!(h);
            l
        }
        None => json!({ "version": 1, "width": w, "height": h, "blocks": [] }),
    };

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO templates (collective_id, group_id, name, master_format_id, milestone_key)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(cid)
    .bind(body.group_id)
    .bind(&body.name)
    .bind(body.master_format_id)
    .bind(&body.milestone_key)
    .fetch_one(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO template_variants (template_id, format_id, is_master, layout)
         VALUES ($1, $2, TRUE, $3)",
    )
    .bind(id)
    .bind(body.master_format_id)
    .bind(&layout)
    .execute(&state.db)
    .await?;
    sqlx::query(
        "INSERT INTO template_versions (template_id, version, snapshot) VALUES ($1, 1, $2)",
    )
    .bind(id)
    .bind(&layout)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "id": id })))
}

async fn format_size(state: &AppState, cid: Uuid, format_id: Uuid) -> AppResult<(i32, i32)> {
    let row: Option<(i32, i32)> = sqlx::query_as(
        "SELECT width, height FROM formats
         WHERE id = $1 AND (collective_id IS NULL OR collective_id = $2)",
    )
    .bind(format_id)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    row.ok_or_else(|| AppError::not_found("format introuvable"))
}

#[derive(Deserialize)]
struct UpdateTemplate {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    milestone_key: Option<String>,
}

async fn update_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateTemplate>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;
    sqlx::query(
        "UPDATE templates SET name = COALESCE($3, name),
                              milestone_key = COALESCE($4, milestone_key),
                              updated_at = now()
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(tid)
    .bind(cid)
    .bind(&body.name)
    .bind(&body.milestone_key)
    .execute(&state.db)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
struct NewVariant {
    format_id: Uuid,
}

/// Cree une declinaison en proposant une adaptation automatique du maitre.
/// **Point de depart, jamais resultat final** : on la corrige ensuite (§9.3).
async fn add_variant(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
    Json(body): Json<NewVariant>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let master: Option<(Value,)> = sqlx::query_as(
        "SELECT v.layout FROM template_variants v JOIN templates t ON t.id = v.template_id
         WHERE v.template_id = $1 AND v.is_master AND t.collective_id = $2",
    )
    .bind(tid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (master,) = master.ok_or_else(|| AppError::not_found("gabarit introuvable"))?;
    let master: templates::Layout =
        serde_json::from_value(master).map_err(|e| AppError::Internal(e.into()))?;

    let (w, h) = format_size(&state, cid, body.format_id).await?;
    let derived = templates::derive_variant(&master, w, h);

    sqlx::query(
        "INSERT INTO template_variants (template_id, format_id, is_master, layout)
         VALUES ($1, $2, FALSE, $3)
         ON CONFLICT (template_id, format_id) DO NOTHING",
    )
    .bind(tid)
    .bind(body.format_id)
    .bind(serde_json::to_value(&derived).unwrap())
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "layout": derived })))
}

#[derive(Deserialize)]
struct SaveVariant {
    layout: Value,
}

/// Chaque declinaison memorise ses propres ajustements sans toucher au maitre.
/// Modifier un gabarit ne casse aucun visuel deja produit : on incremente la
/// version et on en garde un instantane (§9.3).
async fn save_variant(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid, fid)): Path<(Uuid, Uuid, Uuid)>,
    Json(body): Json<SaveVariant>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let owned: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM templates WHERE id = $1 AND collective_id = $2")
            .bind(tid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    owned.ok_or_else(|| AppError::not_found("gabarit introuvable"))?;

    let (w, h) = format_size(&state, cid, fid).await?;
    let mut layout = body.layout;
    layout["width"] = json!(w);
    layout["height"] = json!(h);

    let mut tx = state.db.begin().await?;
    sqlx::query(
        "INSERT INTO template_variants (template_id, format_id, is_master, layout)
         VALUES ($1, $2, FALSE, $3)
         ON CONFLICT (template_id, format_id)
         DO UPDATE SET layout = EXCLUDED.layout, updated_at = now()",
    )
    .bind(tid)
    .bind(fid)
    .bind(&layout)
    .execute(&mut *tx)
    .await?;

    let (version,): (i32,) = sqlx::query_as(
        "UPDATE templates SET version = version + 1, updated_at = now()
         WHERE id = $1 RETURNING version",
    )
    .bind(tid)
    .fetch_one(&mut *tx)
    .await?;

    let variants: Vec<(Uuid, Value)> =
        sqlx::query_as("SELECT format_id, layout FROM template_variants WHERE template_id = $1")
            .bind(tid)
            .fetch_all(&mut *tx)
            .await?;
    let snapshot = json!(variants
        .into_iter()
        .map(|(f, l)| json!({ "format_id": f, "layout": l }))
        .collect::<Vec<_>>());

    sqlx::query(
        "INSERT INTO template_versions (template_id, version, snapshot) VALUES ($1, $2, $3)",
    )
    .bind(tid)
    .bind(version)
    .bind(&snapshot)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(Json(json!({ "version": version })))
}

/// Un gabarit se copie pour servir de base a un autre (§9.3).
async fn duplicate_template(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;
    scope.require_admin()?;

    let src: Option<(String, Option<Uuid>, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT name, group_id, master_format_id, milestone_key FROM templates
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(tid)
    .bind(cid)
    .fetch_optional(&state.db)
    .await?;
    let (name, group_id, master_format_id, milestone_key) =
        src.ok_or_else(|| AppError::not_found("gabarit introuvable"))?;

    let (new_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO templates (collective_id, group_id, name, master_format_id, milestone_key)
         VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(cid)
    .bind(group_id)
    .bind(format!("{name} (copie)"))
    .bind(master_format_id)
    .bind(&milestone_key)
    .fetch_one(&state.db)
    .await?;

    sqlx::query(
        "INSERT INTO template_variants (template_id, format_id, is_master, layout)
         SELECT $2, format_id, is_master, layout FROM template_variants WHERE template_id = $1",
    )
    .bind(tid)
    .bind(new_id)
    .execute(&state.db)
    .await?;

    Ok(Json(json!({ "id": new_id })))
}

// --- Medias ---------------------------------------------------------------

#[derive(Serialize)]
struct AssetRow {
    id: Uuid,
    kind: String,
    filename: String,
    mime: String,
    bytes: i64,
    width: Option<i32>,
    height: Option<i32>,
    duration_ms: Option<i32>,
    tags: Vec<String>,
    group_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct AssetQuery {
    #[serde(default)]
    group_id: Option<Uuid>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    tag: Option<String>,
}

async fn list_assets(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    Query(q): Query<AssetQuery>,
) -> AppResult<Json<Vec<AssetRow>>> {
    let _scope = state.scope(actor, cid).await?;
    #[allow(clippy::type_complexity)]
    let rows: Vec<(Uuid, String, String, String, i64, Option<i32>, Option<i32>, Option<i32>, Vec<String>, Option<Uuid>, DateTime<Utc>)> =
        sqlx::query_as(
            "SELECT id, kind, filename, mime, bytes, width, height, duration_ms, tags, group_id, created_at
             FROM assets
             WHERE collective_id = $1
               AND ($2::uuid IS NULL OR group_id = $2)
               AND ($3::text IS NULL OR kind = $3)
               AND ($4::text IS NULL OR $4 = ANY(tags))
             ORDER BY created_at DESC LIMIT 500",
        )
        .bind(cid)
        .bind(q.group_id)
        .bind(&q.kind)
        .bind(&q.tag)
        .fetch_all(&state.db)
        .await?;

    Ok(Json(
        rows.into_iter()
            .map(
                |(
                    id,
                    kind,
                    filename,
                    mime,
                    bytes,
                    width,
                    height,
                    duration_ms,
                    tags,
                    group_id,
                    created_at,
                )| {
                    AssetRow {
                        id,
                        kind,
                        filename,
                        mime,
                        bytes,
                        width,
                        height,
                        duration_ms,
                        tags,
                        group_id,
                        created_at,
                    }
                },
            )
            .collect(),
    ))
}

/// **Televersement uniquement** : aucune generation par IA, aucun fournisseur
/// a configurer, aucun cout variable (§9.5).
async fn upload_asset(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path(cid): Path<Uuid>,
    mut multipart: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    let scope = state.scope(actor, cid).await?;

    let mut filename = String::from("fichier");
    let mut mime = String::from("application/octet-stream");
    let mut data: Vec<u8> = Vec::new();
    let mut group_id: Option<Uuid> = None;
    let mut tags: Vec<String> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(format!("formulaire illisible : {e}")))?
    {
        match field.name().unwrap_or("") {
            "file" => {
                filename = field.file_name().unwrap_or("fichier").to_string();
                mime = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                data = field
                    .bytes()
                    .await
                    .map_err(|e| AppError::bad_request(format!("lecture du fichier : {e}")))?
                    .to_vec();
            }
            "group_id" => {
                let v = field.text().await.unwrap_or_default();
                group_id = Uuid::parse_str(v.trim()).ok();
            }
            "tags" => {
                let v = field.text().await.unwrap_or_default();
                tags = v
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect();
            }
            _ => {}
        }
    }

    if data.is_empty() {
        return Err(AppError::bad_request("aucun fichier recu"));
    }
    if let Some(gid) = group_id {
        scope.check_group(&state.db, gid).await?;
    }

    let kind = kind_of(&mime);
    let id = Uuid::new_v4();
    let ext = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("bin");
    let key = format!("collectives/{cid}/{kind}/{id}.{ext}");

    state
        .storage
        .put(&key, data.clone(), &mime)
        .await
        .map_err(AppError::Internal)?;

    sqlx::query(
        "INSERT INTO assets (id, collective_id, group_id, kind, filename, storage_key, mime, bytes, tags, uploaded_by)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
    )
    .bind(id)
    .bind(cid)
    .bind(group_id)
    .bind(kind)
    .bind(&filename)
    .bind(&key)
    .bind(&mime)
    .bind(data.len() as i64)
    .bind(&tags)
    .bind(scope.user_id())
    .execute(&state.db)
    .await?;

    Ok(Json(
        json!({ "id": id, "storage_key": key, "bytes": data.len() }),
    ))
}

fn kind_of(mime: &str) -> &'static str {
    match mime.split('/').next().unwrap_or("") {
        "image" => "image",
        "video" => "video",
        "audio" => "audio",
        _ if mime.contains("pdf") => "pdf",
        _ if mime.contains("font") => "font",
        _ => "image",
    }
}

/// URL signee, a duree limitee : c'est ce que consomme le navigateur et la CLI
/// de rendu (§10.1).
async fn asset_url(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, aid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<serde_json::Value>> {
    let _scope = state.scope(actor, cid).await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT storage_key, mime FROM assets WHERE id = $1 AND collective_id = $2")
            .bind(aid)
            .bind(cid)
            .fetch_optional(&state.db)
            .await?;
    let (key, mime) = row.ok_or_else(|| AppError::not_found("media introuvable"))?;
    let url = state
        .storage
        .signed_url(&key, 3600)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(json!({
        "url": url,
        "mime": mime,
        "expires_at": Utc::now() + Duration::hours(1),
    })))
}

/// Les champs automatiques tels qu'ils seront injectes : ce que l'editeur
/// affiche dans sa palette de contenu (§9.1).
async fn preview_fields(
    State(state): State<AppState>,
    Auth(actor): Auth,
    Path((cid, eid)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Value>> {
    let _scope = state.scope(actor, cid).await?;
    crate::routes::events::check_event(&state, cid, eid).await?;
    Ok(Json(templates::resolve_fields(&state.db, eid).await?))
}
