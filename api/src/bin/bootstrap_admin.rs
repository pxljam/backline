//! First instance admin on a fresh database (§3).
//!
//! Production never runs the seed: `RUN_SEED=0` keeps the demo collective out
//! of a real instance. But §3 requires at least one instance admin who signs in
//! with an email and a password, so administration never depends on Telegram —
//! and a database with no such user is a locked door, since passwords are
//! argon2-hashed and cannot be written by hand.
//!
//! This binary opens that door once. It is idempotent: with an instance admin
//! already present it changes nothing and exits 0, so the deploy script can run
//! it on every deployment without thinking about it.
//!
//!   BOOTSTRAP_ADMIN_EMAIL      the address that signs in       (required)
//!   BOOTSTRAP_ADMIN_PASSWORD   at least 10 characters          (required)
//!   BOOTSTRAP_ADMIN_NAME       display name                    (optional)
//!   BOOTSTRAP_COLLECTIVE_NAME  first collective, created and   (optional)
//!   BOOTSTRAP_COLLECTIVE_SLUG  joined as admin                 (optional)

use anyhow::{bail, Context, Result};
use backline::services::provisioning;
use sqlx::PgPool;
use uuid::Uuid;

const MIN_PASSWORD_LEN: usize = 10;

#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let config = backline::Config::from_env()?;
    let db = backline::db::connect(&config.database_url).await?;

    if let Some(existing) = current_admin(&db).await? {
        println!("instance admin already present: {existing} — nothing to do");
        return Ok(());
    }

    let email = env("BOOTSTRAP_ADMIN_EMAIL").context("BOOTSTRAP_ADMIN_EMAIL est obligatoire")?;
    let password =
        env("BOOTSTRAP_ADMIN_PASSWORD").context("BOOTSTRAP_ADMIN_PASSWORD est obligatoire")?;
    if password.chars().count() < MIN_PASSWORD_LEN {
        bail!("BOOTSTRAP_ADMIN_PASSWORD doit faire au moins {MIN_PASSWORD_LEN} caractères");
    }
    let name = env("BOOTSTRAP_ADMIN_NAME").unwrap_or_else(|| "Admin instance".into());

    provisioning::ensure_format_catalog(&db).await?;

    // Reuses the ordinary creation path, so the admin is a user like any other
    // — with an iCal feed and an email identity — plus the one flag.
    let user_id = provisioning::upsert_user(&db, &name, None, "", Some(&email)).await?;
    let hash = backline::auth::hash_password(&password)?;
    sqlx::query("UPDATE users SET password_hash = $2, is_instance_admin = TRUE WHERE id = $1")
        .bind(user_id)
        .bind(hash)
        .execute(&db)
        .await?;
    println!("instance admin created: {email}");

    // An instance admin who belongs to no collective sees an empty application:
    // the collective switcher only lists their own memberships. Creating the
    // first one here spares a fresh deployment that dead end.
    if let Some(collective_name) = env("BOOTSTRAP_COLLECTIVE_NAME") {
        let slug = env("BOOTSTRAP_COLLECTIVE_SLUG").unwrap_or_else(|| slugify(&collective_name));
        match existing_collective(&db, &slug).await? {
            Some(id) => {
                provisioning::add_member(&db, id, user_id, "admin").await?;
                println!("collective already present: {slug} — admin attached");
            }
            None => {
                let id = provisioning::create_collective(&db, &slug, &collective_name).await?;
                provisioning::add_member(&db, id, user_id, "admin").await?;
                println!("collective created: {slug}");
            }
        }
    }

    Ok(())
}

fn env(key: &str) -> Option<String> {
    std::env::var(key).ok().filter(|v| !v.trim().is_empty())
}

async fn current_admin(db: &PgPool) -> Result<Option<String>> {
    let row: Option<(Option<String>, String)> = sqlx::query_as(
        "SELECT email, display_name FROM users WHERE is_instance_admin ORDER BY created_at LIMIT 1",
    )
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(email, name)| email.unwrap_or(name)))
}

async fn existing_collective(db: &PgPool, slug: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM collectives WHERE slug = $1")
        .bind(slug)
        .fetch_optional(db)
        .await?;
    Ok(row.map(|(id,)| id))
}

/// Lowercase ASCII, one dash between words. Good enough for a name typed once
/// at deployment; anything else is set from the interface afterwards.
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    out.trim_end_matches('-').to_string()
}
