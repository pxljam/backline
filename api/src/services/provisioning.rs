//! Creation d'un collectif. Manuelle, faite par un admin d'instance (§2) —
//! il n'y a jamais de deploiement par collectif.
//!
//! Un collectif nait **utilisable** : types d'evenements avec leurs timelines,
//! charte d'exemple, flux iCal. Aucune de ces valeurs n'est codee en dur dans
//! l'interface : ce sont des donnees, editables ensuite (§8, §11.1).

use crate::error::AppResult;
use crate::services::{comms, formats};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

/// Catalogue de formats au niveau instance. Idempotent.
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
        // Une residence est UN evenement sur une plage, pas une serie (§6).
        ("residency", "Residence", false, true),
        // Un stream ne passe pas par une opportunite : pas de lieu a negocier.
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

/// Jeu de tokens d'exemple, volontairement sobre (§8, dependance d'entree).
/// Saisir les vraies valeurs de Bonsoir Techno ne demandera aucune
/// modification de code.
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
        // `stack` sert de repli tant que les vraies polices ne sont pas televersees.
        .bind(json!({
            "family": family,
            "weight": weight,
            "stack": format!("{family}, Helvetica, Arial, sans-serif")
        }))
        .bind(i as i32)
        .execute(db)
        .await?;
    }

    // Regles de charte : placement du logo, casse des titres, mentions.
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
