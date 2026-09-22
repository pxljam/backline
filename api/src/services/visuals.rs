//! Generation des visuels fixes (§9.4).
//!
//! « Generer les visuels » produit **en une passe** tous les formats du plan de
//! com, champs automatiques deja remplis, sans ouvrir l'editeur.
//!
//! Le rendu lui-meme part au service `stills`, qui execute les composants
//! Remotion de `layout/`. Rust n'a aucune idee de ce a quoi ressemble un
//! visuel : c'est ce qui garantit qu'il n'existe **qu'une implementation de la
//! mise en page** (§15).

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

/// Charte effective : tokens du collectif, surcharges par ceux du groupe.
pub async fn resolve_brand(
    db: &PgPool,
    collective_id: Uuid,
    group_id: Option<Uuid>,
) -> AppResult<Value> {
    let mut tokens = serde_json::Map::new();

    let load = |rows: Vec<(String, String, Value)>, map: &mut serde_json::Map<String, Value>| {
        for (kind, key, value) in rows {
            let entry = map
                .entry(kind)
                .or_insert_with(|| Value::Object(serde_json::Map::new()));
            if let Some(obj) = entry.as_object_mut() {
                obj.insert(key, value);
            }
        }
    };

    let base: Vec<(String, String, Value)> = sqlx::query_as(
        "SELECT t.kind, t.key, t.value FROM brand_tokens t JOIN brands b ON b.id = t.brand_id
         WHERE b.collective_id = $1 AND b.group_id IS NULL ORDER BY t.position",
    )
    .bind(collective_id)
    .fetch_all(db)
    .await?;
    load(base, &mut tokens);

    if let Some(gid) = group_id {
        let over: Vec<(String, String, Value)> = sqlx::query_as(
            "SELECT t.kind, t.key, t.value FROM brand_tokens t JOIN brands b ON b.id = t.brand_id
             WHERE b.group_id = $1 ORDER BY t.position",
        )
        .bind(gid)
        .fetch_all(db)
        .await?;
        load(over, &mut tokens);
    }

    Ok(Value::Object(tokens))
}

/// Choisit le gabarit a utiliser pour un jalon et un format.
///
/// Priorite : gabarit du groupe porteur rattache au jalon, puis gabarit du
/// collectif rattache au jalon, puis n'importe quel gabarit couvrant le format.
async fn pick_template(
    db: &PgPool,
    collective_id: Uuid,
    host_group_id: Option<Uuid>,
    milestone_key: &str,
    format_id: Uuid,
) -> AppResult<Option<(Uuid, i32, Value)>> {
    let row: Option<(Uuid, i32, Value)> = sqlx::query_as(
        "SELECT t.id, t.version, v.layout
         FROM templates t JOIN template_variants v ON v.template_id = t.id
         WHERE t.collective_id = $1 AND t.archived = FALSE AND v.format_id = $2
         ORDER BY
           (t.group_id IS NOT DISTINCT FROM $3) DESC,
           (t.milestone_key = $4) DESC,
           t.updated_at DESC
         LIMIT 1",
    )
    .bind(collective_id)
    .bind(format_id)
    .bind(host_group_id)
    .bind(milestone_key)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

/// Produit tous les visuels manquants du plan de com d'un evenement.
/// Renvoie (produits, en echec).
pub async fn generate_for_event(state: &AppState, event_id: Uuid) -> AppResult<(usize, usize)> {
    let (collective_id, host_group_id): (Uuid, Option<Uuid>) =
        sqlx::query_as("SELECT collective_id, host_group_id FROM events WHERE id = $1")
            .bind(event_id)
            .fetch_one(&state.db)
            .await?;

    let fields = crate::services::templates::resolve_fields(&state.db, event_id).await?;
    let brand = resolve_brand(&state.db, collective_id, host_group_id).await?;

    let tasks: Vec<(Uuid, String, Value)> = sqlx::query_as(
        "SELECT id, milestone_key, formats FROM publication_tasks WHERE event_id = $1",
    )
    .bind(event_id)
    .fetch_all(&state.db)
    .await?;

    let mut ok = 0usize;
    let mut failed = 0usize;

    for (task_id, milestone_key, formats) in tasks {
        let keys: Vec<String> = serde_json::from_value(formats).unwrap_or_default();
        for key in keys {
            let format: Option<(Uuid, i32, i32, String)> = sqlx::query_as(
                "SELECT id, width, height, kind FROM formats
                 WHERE key = $1 AND (collective_id IS NULL OR collective_id = $2)
                 ORDER BY collective_id NULLS LAST LIMIT 1",
            )
            .bind(&key)
            .bind(collective_id)
            .fetch_optional(&state.db)
            .await?;
            let Some((format_id, width, height, _kind)) = format else {
                continue;
            };

            // Deja rendu : on ne refait pas le travail.
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT status FROM publication_assets
                 WHERE publication_task_id = $1 AND format_id = $2",
            )
            .bind(task_id)
            .bind(format_id)
            .fetch_optional(&state.db)
            .await?;
            if matches!(existing.as_ref().map(|(s,)| s.as_str()), Some("ready")) {
                continue;
            }

            let picked = pick_template(
                &state.db,
                collective_id,
                host_group_id,
                &milestone_key,
                format_id,
            )
            .await?;
            let Some((template_id, template_version, layout)) = picked else {
                record_failure(
                    &state.db,
                    task_id,
                    format_id,
                    "aucun gabarit ne couvre ce format",
                )
                .await?;
                failed += 1;
                continue;
            };

            match render_still(state, &layout, &brand, &fields, width, height).await {
                Ok(bytes) => {
                    let asset_id = store_render(
                        state,
                        collective_id,
                        host_group_id,
                        &format!("{milestone_key}-{key}.png"),
                        bytes,
                    )
                    .await?;
                    sqlx::query(
                        "INSERT INTO publication_assets
                            (publication_task_id, format_id, template_id, template_version, asset_id, status)
                         VALUES ($1, $2, $3, $4, $5, 'ready')
                         ON CONFLICT (publication_task_id, format_id)
                         DO UPDATE SET asset_id = EXCLUDED.asset_id, status = 'ready',
                                       template_id = EXCLUDED.template_id,
                                       template_version = EXCLUDED.template_version, error = NULL",
                    )
                    .bind(task_id)
                    .bind(format_id)
                    .bind(template_id)
                    .bind(template_version)
                    .bind(asset_id)
                    .execute(&state.db)
                    .await?;
                    ok += 1;
                }
                Err(e) => {
                    record_failure(&state.db, task_id, format_id, &e.detail()).await?;
                    failed += 1;
                }
            }
        }

        // Une tache dont tous les visuels sont prets passe de `brouillon` a
        // `pret` — mais jamais a `publie` : un humain valide toujours (§11.1).
        sqlx::query(
            "UPDATE publication_tasks SET status = 'ready', updated_at = now()
             WHERE id = $1 AND status = 'draft'
               AND NOT EXISTS (
                 SELECT 1 FROM publication_assets
                 WHERE publication_task_id = $1 AND status <> 'ready')
               AND EXISTS (SELECT 1 FROM publication_assets WHERE publication_task_id = $1)",
        )
        .bind(task_id)
        .execute(&state.db)
        .await?;
    }

    Ok((ok, failed))
}

async fn record_failure(db: &PgPool, task_id: Uuid, format_id: Uuid, error: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO publication_assets (publication_task_id, format_id, status, error)
         VALUES ($1, $2, 'failed', $3)
         ON CONFLICT (publication_task_id, format_id)
         DO UPDATE SET status = 'failed', error = EXCLUDED.error",
    )
    .bind(task_id)
    .bind(format_id)
    .bind(error)
    .execute(db)
    .await?;
    Ok(())
}

async fn store_render(
    state: &AppState,
    collective_id: Uuid,
    group_id: Option<Uuid>,
    filename: &str,
    bytes: Vec<u8>,
) -> AppResult<Uuid> {
    let id = Uuid::new_v4();
    let key = format!("collectives/{collective_id}/render/{id}.png");
    state
        .storage
        .put(&key, bytes.clone(), "image/png")
        .await
        .map_err(AppError::Internal)?;

    sqlx::query(
        "INSERT INTO assets
            (id, collective_id, group_id, kind, filename, storage_key, mime, bytes, purge_after)
         VALUES ($1, $2, $3, 'render', $4, $5, 'image/png', $6, $7)",
    )
    .bind(id)
    .bind(collective_id)
    .bind(group_id)
    .bind(filename)
    .bind(&key)
    .bind(bytes.len() as i64)
    // Purge a six mois : un rendu se regenere a l'identique depuis sa
    // description (§15). Les medias televerses, eux, ne sont jamais purges.
    .bind(Utc::now() + Duration::days(183))
    .execute(&state.db)
    .await?;

    Ok(id)
}

async fn render_still(
    state: &AppState,
    layout: &Value,
    brand: &Value,
    fields: &Value,
    width: i32,
    height: i32,
) -> AppResult<Vec<u8>> {
    let url = state
        .config
        .stills_url
        .as_ref()
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("service de rendu non configure")))?;

    let body = json!({
        "layout": layout,
        "brand": brand,
        "data": fields,
        "width": width,
        "height": height,
    });

    let resp = reqwest::Client::new()
        .post(format!("{}/render", url.trim_end_matches('/')))
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await
        .map_err(|e| AppError::Internal(anyhow::anyhow!("service de rendu injoignable : {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(anyhow::anyhow!(
            "rendu refuse ({status}) : {text}"
        )));
    }

    Ok(resp
        .bytes()
        .await
        .map_err(|e| AppError::Internal(e.into()))?
        .to_vec())
}
