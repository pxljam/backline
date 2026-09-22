//! Gabarits et declinaisons (§9).
//!
//! **La mise en page est une description JSON, seule source de verite** (§15).
//! Rust ne la dessine jamais : il la stocke, la decline et y injecte les champs
//! automatiques. Le dessin appartient aux composants Remotion de `layout/`,
//! partages par l'editeur, le rendu des visuels fixes et la CLI video.
//!
//! Convention de coordonnees, choisie pour que le verrouillage de ratio et la
//! declinaison multi-format soient naturels :
//!
//! - `x`, `w` sont des fractions de la **largeur** du canevas ;
//! - `y`, `h` des fractions de la **hauteur** ;
//! - toutes les **tailles** (police, trait, rayon) sont des fractions de la
//!   **largeur**. Deux formats de meme largeur (1080×1350 et 1080×1920) rendent
//!   donc un texte strictement identique — ce qui est le cas courant.

use crate::error::AppResult;
use chrono_tz::Europe::Paris;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    #[serde(default = "one")]
    pub version: u32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub background: Option<Value>,
    #[serde(default)]
    pub blocks: Vec<Block>,
    /// Duree en images, pour une composition animee. Absente = image fixe :
    /// « la seule difference entre une image et une video devient la duree ».
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_in_frames: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<u32>,
}

fn one() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    /// text | image | video | logo | shape | gradient | group
    #[serde(rename = "type")]
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub z: i32,
    #[serde(default)]
    pub props: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timing: Option<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub animations: Vec<Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Block>,
}

/// Adaptation automatique d'un format maitre vers une declinaison (§9.3).
///
/// C'est **un point de depart, jamais un resultat final** : l'app propose, on
/// corrige a la main, et la declinaison garde ses propres ajustements.
///
/// La largeur relative et les tailles sont conservees telles quelles ; seules
/// les hauteurs sont corrigees pour preserver la proportion visible des blocs,
/// autour de leur centre.
pub fn derive_variant(master: &Layout, width: i32, height: i32) -> Layout {
    let old_hw = master.height as f64 / master.width as f64;
    let new_hw = height as f64 / width as f64;
    let f = old_hw / new_hw;

    let blocks = master.blocks.iter().map(|b| adapt(b, f)).collect();

    Layout {
        version: master.version,
        width,
        height,
        background: master.background.clone(),
        blocks,
        duration_in_frames: master.duration_in_frames,
        fps: master.fps,
    }
}

fn adapt(b: &Block, f: f64) -> Block {
    let center_y = b.y + b.h / 2.0;
    let h = (b.h * f).clamp(0.01, 1.0);
    let y = (center_y - h / 2.0).clamp(0.0, 1.0 - h);
    Block {
        y,
        h,
        children: b.children.iter().map(|c| adapt(c, f)).collect(),
        ..b.clone()
    }
}

/// Champs automatiques de l'evenement (§9.1). Deposes sur le canevas, ils se
/// remplissent tout seuls a la generation.
pub async fn resolve_fields(db: &PgPool, event_id: Uuid) -> AppResult<Value> {
    #[allow(clippy::type_complexity)]
    let row: (
        String,
        chrono::DateTime<chrono::Utc>,
        Option<chrono::DateTime<chrono::Utc>>,
        bool,
        String,
        Option<String>,
        Option<String>,
        String,
    ) = sqlx::query_as(
        "SELECT e.title, e.starts_at, e.ends_at, e.set_times_public, c.name, v.name, v.city, t.key
             FROM events e
             JOIN collectives c ON c.id = e.collective_id
             JOIN event_types t ON t.id = e.event_type_id
             LEFT JOIN venues v ON v.id = e.venue_id
             WHERE e.id = $1",
    )
    .bind(event_id)
    .fetch_one(db)
    .await?;

    let (title, starts_at, ends_at, set_times_public, collective, venue, city, type_key) = row;
    let local = starts_at.with_timezone(&Paris);

    #[allow(clippy::type_complexity)]
    let parts: Vec<(
        Option<String>,
        Option<String>,
        Option<chrono::NaiveTime>,
        Option<chrono::NaiveTime>,
    )> = sqlx::query_as(
        "SELECT g.name, COALESCE(u.stage_name, u.display_name), p.slot_start, p.slot_end
             FROM participations p
             LEFT JOIN groups g ON g.id = p.group_id
             LEFT JOIN users u ON u.id = p.user_id
             WHERE p.event_id = $1 AND p.status <> 'declined'
             ORDER BY p.position",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    // Tant que les creneaux ne sont pas publiables, les gabarits affichent
    // l'ordre du line-up **sans heures** : aucun visuel ne sort avec un horaire
    // provisoire (§6).
    let line_up: Vec<Value> = parts
        .iter()
        .map(|(g, u, s, e)| {
            let name = g.clone().or_else(|| u.clone()).unwrap_or_default();
            let slot = match (set_times_public, s, e) {
                (true, Some(s), Some(e)) => {
                    Some(format!("{}–{}", s.format("%H:%M"), e.format("%H:%M")))
                }
                (true, Some(s), None) => Some(s.format("%H:%M").to_string()),
                _ => None,
            };
            json!({ "name": name, "slot": slot })
        })
        .collect();

    let handles: Vec<(String, String)> = sqlx::query_as(
        "SELECT platform, handle FROM social_accounts sa
         JOIN events e ON e.collective_id = sa.collective_id
         WHERE e.id = $1 AND sa.group_id IS NULL",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    let platforms: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT platform, url FROM event_stream_platforms WHERE event_id = $1")
            .bind(event_id)
            .fetch_all(db)
            .await?;

    Ok(json!({
        "event": {
            "title": title,
            "type": type_key,
            "date": local.format("%d.%m.%Y").to_string(),
            "date_long": format_date_fr(local),
            "weekday": weekday_fr(local),
            "time": local.format("%H:%M").to_string(),
            "end_time": ends_at.map(|e| e.with_timezone(&Paris).format("%H:%M").to_string()),
            "set_times_public": set_times_public,
        },
        "collective": { "name": collective },
        "venue": { "name": venue, "city": city },
        "line_up": line_up,
        "line_up_text": line_up
            .iter()
            .map(|l| {
                let name = l["name"].as_str().unwrap_or("");
                match l["slot"].as_str() {
                    Some(s) => format!("{name} — {s}"),
                    None => name.to_string(),
                }
            })
            .collect::<Vec<_>>()
            .join("\n"),
        "handles": handles
            .iter()
            .map(|(p, h)| json!({ "platform": p, "handle": h }))
            .collect::<Vec<_>>(),
        "stream": {
            "platforms": platforms
                .iter()
                .map(|(p, u)| json!({ "platform": p, "url": u }))
                .collect::<Vec<_>>()
        }
    }))
}

fn weekday_fr(d: chrono::DateTime<chrono_tz::Tz>) -> String {
    use chrono::Datelike;
    [
        "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche",
    ][d.weekday().num_days_from_monday() as usize]
        .to_string()
}

fn format_date_fr(d: chrono::DateTime<chrono_tz::Tz>) -> String {
    use chrono::Datelike;
    let months = [
        "janvier",
        "fevrier",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "aout",
        "septembre",
        "octobre",
        "novembre",
        "decembre",
    ];
    format!(
        "{} {} {}",
        d.day(),
        months[(d.month() - 1) as usize],
        d.year()
    )
}

/// Gabarit d'annonce livre avec l'instance. Il couvre l'usage courant — refaire
/// la meme affiche avec d'autres noms — sans jamais ouvrir l'editeur (§9.4).
pub async fn seed_default_template(
    db: &PgPool,
    collective_id: Uuid,
    group_id: Option<Uuid>,
) -> AppResult<Uuid> {
    let (master_format_id, w, h): (Uuid, i32, i32) = sqlx::query_as(
        "SELECT id, width, height FROM formats WHERE collective_id IS NULL AND key = 'ig_portrait'",
    )
    .fetch_one(db)
    .await?;

    let master = Layout {
        version: 1,
        width: w,
        height: h,
        background: Some(json!({ "type": "color", "token": "background" })),
        blocks: vec![
            Block {
                id: "bandeau".into(),
                kind: "shape".into(),
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 0.34,
                rotation: 0.0,
                opacity: Some(1.0),
                locked: false,
                z: 0,
                props: json!({ "shape": "rect", "fillToken": "primary" }),
                timing: None,
                animations: vec![],
                children: vec![],
            },
            Block {
                id: "collectif".into(),
                kind: "text".into(),
                x: 0.08,
                y: 0.07,
                w: 0.84,
                h: 0.07,
                rotation: 0.0,
                opacity: Some(1.0),
                locked: false,
                z: 1,
                props: json!({
                    "content": "{{collective.name}}",
                    "fontToken": "subtitle",
                    "colorToken": "text_inverse",
                    "size": 0.042,
                    "align": "left",
                    "transform": "uppercase",
                    "tracking": 0.12
                }),
                timing: None,
                animations: vec![],
                children: vec![],
            },
            Block {
                id: "titre".into(),
                kind: "text".into(),
                x: 0.08,
                y: 0.4,
                w: 0.84,
                h: 0.22,
                rotation: 0.0,
                opacity: Some(1.0),
                locked: false,
                z: 2,
                props: json!({
                    "content": "{{event.title}}",
                    "fontToken": "title",
                    "colorToken": "text",
                    "size": 0.095,
                    "align": "left",
                    "lineHeight": 1.02,
                    "transform": "uppercase"
                }),
                timing: None,
                animations: vec![],
                children: vec![],
            },
            Block {
                id: "lineup".into(),
                kind: "text".into(),
                x: 0.08,
                y: 0.64,
                w: 0.84,
                h: 0.16,
                rotation: 0.0,
                opacity: Some(1.0),
                locked: false,
                z: 3,
                props: json!({
                    "content": "{{line_up_text}}",
                    "fontToken": "body",
                    "colorToken": "text",
                    "size": 0.045,
                    "align": "left",
                    "lineHeight": 1.35
                }),
                timing: None,
                animations: vec![],
                children: vec![],
            },
            Block {
                id: "infos".into(),
                kind: "text".into(),
                x: 0.08,
                y: 0.84,
                w: 0.84,
                h: 0.1,
                rotation: 0.0,
                opacity: Some(1.0),
                locked: false,
                z: 4,
                props: json!({
                    "content": "{{event.date_long}} · {{event.time}}\n{{venue.name}} — {{venue.city}}",
                    "fontToken": "body",
                    "colorToken": "accent",
                    "size": 0.038,
                    "align": "left",
                    "lineHeight": 1.3
                }),
                timing: None,
                animations: vec![],
                children: vec![],
            },
        ],
        duration_in_frames: None,
        fps: None,
    };

    let (template_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO templates (collective_id, group_id, name, master_format_id, milestone_key)
         VALUES ($1, $2, 'Annonce', $3, 'j-30') RETURNING id",
    )
    .bind(collective_id)
    .bind(group_id)
    .bind(master_format_id)
    .fetch_one(db)
    .await?;

    sqlx::query(
        "INSERT INTO template_variants (template_id, format_id, is_master, layout)
         VALUES ($1, $2, TRUE, $3)",
    )
    .bind(template_id)
    .bind(master_format_id)
    .bind(serde_json::to_value(&master).unwrap())
    .execute(db)
    .await?;

    // Declinaisons automatiques sur les formats du meme jalon.
    for key in ["ig_square", "ig_story"] {
        let (fid, fw, fh): (Uuid, i32, i32) = sqlx::query_as(
            "SELECT id, width, height FROM formats WHERE collective_id IS NULL AND key = $1",
        )
        .bind(key)
        .fetch_one(db)
        .await?;
        let derived = derive_variant(&master, fw, fh);
        sqlx::query(
            "INSERT INTO template_variants (template_id, format_id, is_master, layout)
             VALUES ($1, $2, FALSE, $3)",
        )
        .bind(template_id)
        .bind(fid)
        .bind(serde_json::to_value(&derived).unwrap())
        .execute(db)
        .await?;
    }

    sqlx::query(
        "INSERT INTO template_versions (template_id, version, snapshot) VALUES ($1, 1, $2)",
    )
    .bind(template_id)
    .bind(serde_json::to_value(&master).unwrap())
    .execute(db)
    .await?;

    Ok(template_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn master() -> Layout {
        Layout {
            version: 1,
            width: 1080,
            height: 1350,
            background: None,
            blocks: vec![Block {
                id: "b".into(),
                kind: "image".into(),
                x: 0.1,
                y: 0.4,
                w: 0.8,
                h: 0.2,
                rotation: 0.0,
                opacity: None,
                locked: false,
                z: 0,
                props: json!({}),
                timing: None,
                animations: vec![],
                children: vec![],
            }],
            duration_in_frames: None,
            fps: None,
        }
    }

    #[test]
    fn la_declinaison_adopte_le_format_cible_sans_negociation() {
        let v = derive_variant(&master(), 1080, 1920);
        assert_eq!((v.width, v.height), (1080, 1920));
    }

    #[test]
    fn la_proportion_visible_d_un_bloc_est_preservee() {
        let m = master();
        let b0 = &m.blocks[0];
        let aspect_before = (b0.h * m.height as f64) / (b0.w * m.width as f64);

        let v = derive_variant(&m, 1080, 1920);
        let b1 = &v.blocks[0];
        let aspect_after = (b1.h * v.height as f64) / (b1.w * v.width as f64);

        assert!((aspect_before - aspect_after).abs() < 1e-9);
    }

    #[test]
    fn le_bloc_reste_centre_sur_lui_meme() {
        let m = master();
        let before = m.blocks[0].y + m.blocks[0].h / 2.0;
        let v = derive_variant(&m, 1080, 1920);
        let after = v.blocks[0].y + v.blocks[0].h / 2.0;
        assert!((before - after).abs() < 1e-9);
    }

    #[test]
    fn un_bloc_adapte_ne_sort_jamais_du_cadre() {
        let mut m = master();
        m.blocks[0].y = 0.9;
        m.blocks[0].h = 0.09;
        let v = derive_variant(&m, 1080, 720);
        let b = &v.blocks[0];
        assert!(b.y >= 0.0 && b.y + b.h <= 1.0 + 1e-9, "y={} h={}", b.y, b.h);
    }
}
