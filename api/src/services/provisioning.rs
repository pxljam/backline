//! Creating a collective. Manual, done by an instance admin (§2) — there is
//! never a deployment per collective.
//!
//! A collective is born **usable**: event types with their timelines, a sample
//! brand, iCal feeds. None of these values is hard-coded in the interface: they
//! are data, editable afterwards (§8, §11.1).

use crate::error::AppResult;
use crate::services::{comms, formats};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Instance-level format catalogue. Idempotent.
pub async fn ensure_format_catalog(db: &PgPool) -> AppResult<()> {
    for (i, f) in formats::CATALOG.iter().enumerate() {
        sqlx::query(
            "INSERT INTO formats (collective_id, key, platform, label, width, height, kind, position)
             VALUES (NULL, $1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT (key) WHERE collective_id IS NULL DO NOTHING",
        )
        .bind(f.key)
        .bind(f.platform)
        .bind(f.label)
        .bind(f.width)
        .bind(f.height)
        .bind(f.kind)
        .bind(i as i32)
        .execute(db)
        .await?;
    }
    Ok(())
}

pub async fn create_collective(db: &PgPool, slug: &str, name: &str) -> AppResult<Uuid> {
    ensure_format_catalog(db).await?;

    let (collective_id,): (Uuid,) =
        sqlx::query_as("INSERT INTO collectives (slug, name) VALUES ($1, $2) RETURNING id")
            .bind(slug)
            .bind(name)
            .fetch_one(db)
            .await?;

    seed_event_types(db, collective_id).await?;
    seed_brand(db, collective_id, name, None).await?;
    crate::services::ical::ensure_token(db, "collective", collective_id).await?;

    Ok(collective_id)
}

pub async fn seed_event_types(db: &PgPool, collective_id: Uuid) -> AppResult<()> {
    let types: [(&str, &str, bool, bool); 4] = [
        ("dj_night", "Soiree DJ electro", true, false),
        ("concert", "Concert", true, false),
        // A residency is ONE event over a range, not a series (§6).
        ("residency", "Residence", false, true),
        // A stream does not go through an opportunity: no venue to negotiate.
        ("stream", "Stream live", false, false),
    ];

    for (i, (key, label, requires_venue, is_range)) in types.iter().enumerate() {
        let milestones = serde_json::to_value(comms::default_timeline(key)).unwrap();
        let logistics = json!(comms::default_logistics(key)
            .into_iter()
            .map(|(label, qty)| json!({ "label": label, "quantity": qty }))
            .collect::<Vec<_>>());

        sqlx::query(
            "INSERT INTO event_types
                (collective_id, key, label, comms_milestones, default_logistics_slots,
                 requires_venue, is_range, position)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (collective_id, key) DO NOTHING",
        )
        .bind(collective_id)
        .bind(key)
        .bind(label)
        .bind(&milestones)
        .bind(&logistics)
        .bind(requires_venue)
        .bind(is_range)
        .bind(i as i32)
        .execute(db)
        .await?;
    }
    Ok(())
}

/// A deliberately plain set of sample tokens (§8, input dependency). Entering
/// Bonsoir Techno's real values will require no code change.
pub async fn seed_brand(
    db: &PgPool,
    collective_id: Uuid,
    name: &str,
    group_id: Option<Uuid>,
) -> AppResult<Uuid> {
    let (brand_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO brands (collective_id, group_id, name) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(collective_id)
    .bind(group_id)
    .bind(name)
    .fetch_one(db)
    .await?;

    let colors: [(&str, &str, &str); 6] = [
        ("primary", "Primaire", "#111111"),
        ("secondary", "Secondaire", "#4A4A4A"),
        ("accent", "Accent", "#E4573D"),
        ("background", "Fond", "#F5F3EF"),
        ("text", "Texte", "#111111"),
        ("text_inverse", "Texte inverse", "#F5F3EF"),
    ];
    for (i, (key, label, hex)) in colors.iter().enumerate() {
        sqlx::query(
            "INSERT INTO brand_tokens (brand_id, kind, key, label, value, position)
             VALUES ($1, 'color', $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(brand_id)
        .bind(key)
        .bind(label)
        .bind(json!({ "hex": hex }))
        .bind(i as i32)
        .execute(db)
        .await?;
    }

    let fonts: [(&str, &str, &str, &str); 3] = [
        ("title", "Titre", "Inter", "800"),
        ("subtitle", "Sous-titre", "Inter", "600"),
        ("body", "Corps", "Inter", "400"),
    ];
    for (i, (key, label, family, weight)) in fonts.iter().enumerate() {
        sqlx::query(
            "INSERT INTO brand_tokens (brand_id, kind, key, label, value, position)
             VALUES ($1, 'font', $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(brand_id)
        .bind(key)
        .bind(label)
        // `stack` is the fallback until the real fonts are uploaded.
        .bind(json!({
            "family": family,
            "weight": weight,
            "stack": format!("{family}, Helvetica, Arial, sans-serif")
        }))
        .bind(i as i32)
        .execute(db)
        .await?;
    }

    // Brand rules: logo placement, title casing, legal mentions.
    for (i, (key, label, value)) in [
        (
            "logo_placement",
            "Placement du logo",
            json!({ "corner": "bottom-left", "margin": 0.06 }),
        ),
        (
            "title_case",
            "Casse des titres",
            json!({ "transform": "uppercase" }),
        ),
        (
            "safe_area",
            "Zone de securite",
            json!({ "top": 0.14, "bottom": 0.18, "x": 0.06 }),
        ),
    ]
    .iter()
    .enumerate()
    {
        sqlx::query(
            "INSERT INTO brand_tokens (brand_id, kind, key, label, value, position)
             VALUES ($1, 'rule', $2, $3, $4, $5) ON CONFLICT DO NOTHING",
        )
        .bind(brand_id)
        .bind(key)
        .bind(label)
        .bind(value)
        .bind(i as i32)
        .execute(db)
        .await?;
    }

    Ok(brand_id)
}
