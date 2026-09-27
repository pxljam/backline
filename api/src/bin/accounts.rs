//! Accounts from the command line (§3).
//!
//! The counterpart of the instance screen, for when there is no browser at
//! hand: on the server, right after a deployment, or to hand someone a fresh
//! link without logging in. It talks to the database directly, so it runs where
//! the database is:
//!
//!   docker compose run --rm api accounts list
//!   docker compose run --rm api accounts create --name "Ada" --email ada@…
//!   docker compose run --rm api accounts invite ada@…
//!   docker compose run --rm api accounts admin ada@… --on
//!
//! The render CLI deliberately holds no user session (`cli/src/config.ts`), so
//! account creation does not belong there.

// Same reasoning as `lib.rs`: a query result is read as a typed tuple rather
// than a dedicated struct.
#![allow(clippy::type_complexity)]

use anyhow::{bail, Context, Result};
use backline::services::{invitations, provisioning};
use sqlx::PgPool;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    backline::init_tracing();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("help");

    if command == "help" || command == "--help" || command == "-h" {
        help();
        return Ok(());
    }

    let config = backline::Config::from_env()?;
    let db = backline::db::connect(&config.database_url).await?;

    match command {
        "list" => list(&db).await,
        "create" => create(&db, &config, &args).await,
        "invite" => invite(&db, &config, &args).await,
        "admin" => admin(&db, &args).await,
        other => {
            help();
            bail!("unknown command: {other}");
        }
    }
}

fn help() {
    println!(
        r#"accounts — create and invite people (no public sign-up, §3)

  accounts list
  accounts create --name <name> [--email <email>] [--phone <phone>]
                  [--stage <stage name>] [--instance-admin]
                  [--collective <slug>] [--role admin|member]
  accounts invite <email or id>
  accounts admin  <email or id> --on|--off

`create` and `invite` both print à single-use invitation link, valid {days}
days. The previous link of that person stops working."#,
        days = invitations::VALIDITY_DAYS
    );
}

/// `--name value`. Flags without a value are read with `flag`.
fn option(args: &[String], name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == &format!("--{name}"))?;
    args.get(i + 1)
        .filter(|v| !v.starts_with("--"))
        .map(|v| v.to_string())
}

fn flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == &format!("--{name}"))
}

async fn list(db: &PgPool) -> Result<()> {
    let rows: Vec<(String, Option<String>, bool, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT u.display_name, u.email, u.is_instance_admin,
                u.password_hash IS NOT NULL OR u.telegram_id IS NOT NULL,
                EXISTS (SELECT 1 FROM invitations i
                        WHERE i.user_id = u.id AND i.used_at IS NULL AND i.expires_at > now()),
                (SELECT string_agg(c.name, ', ' ORDER BY c.name) FROM memberships m
                 JOIN collectives c ON c.id = m.collective_id WHERE m.user_id = u.id)
         FROM users u ORDER BY u.is_instance_admin DESC, u.display_name",
    )
    .fetch_all(db)
    .await?;

    if rows.is_empty() {
        println!("no account yet — start with: accounts create --name … --email …");
        return Ok(());
    }
    for (name, email, is_admin, can_sign_in, pending, collectives) in rows {
        let mut tags = Vec::new();
        if is_admin {
            tags.push("instance admin".to_string());
        }
        if !can_sign_in {
            tags.push("never signed in".to_string());
        }
        if pending {
            tags.push("invitation pending".to_string());
        }
        if let Some(c) = collectives {
            tags.push(c);
        }
        println!(
            "{name:<24} {:<32} {}",
            email.unwrap_or_else(|| "—".into()),
            tags.join(" · ")
        );
    }
    Ok(())
}

async fn create(db: &PgPool, config: &backline::Config, args: &[String]) -> Result<()> {
    let name = option(args, "name").context("--name est obligatoire")?;
    let email = option(args, "email");
    let phone = option(args, "phone").unwrap_or_default();
    let stage = option(args, "stage");

    provisioning::ensure_format_catalog(db).await?;
    let user_id = provisioning::upsert_user(db, &name, stage.as_deref(), &phone, email.as_deref())
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;

    if flag(args, "instance-admin") {
        sqlx::query("UPDATE users SET is_instance_admin = TRUE WHERE id = $1")
            .bind(user_id)
            .execute(db)
            .await?;
        println!("instance admin granted");
    }

    if let Some(slug) = option(args, "collective") {
        let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM collectives WHERE slug = $1")
            .bind(&slug)
            .fetch_optional(db)
            .await?;
        let (collective_id,) = row.with_context(|| format!("collectif introuvable : {slug}"))?;
        let role = match option(args, "role").as_deref() {
            Some("admin") => "admin",
            _ => "member",
        };
        provisioning::add_member(db, collective_id, user_id, role)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        println!("attached to {slug} as {role}");
    }

    print_link(db, config, user_id).await
}

async fn invite(db: &PgPool, config: &backline::Config, args: &[String]) -> Result<()> {
    let who = args
        .get(1)
        .context("usage: accounts invite <email or id>")?;
    let user_id = resolve(db, who).await?;
    print_link(db, config, user_id).await
}

async fn admin(db: &PgPool, args: &[String]) -> Result<()> {
    let who = args
        .get(1)
        .context("usage: accounts admin <email or id> --on|--off")?;
    let user_id = resolve(db, who).await?;
    let on = match (flag(args, "on"), flag(args, "off")) {
        (true, false) => true,
        (false, true) => false,
        _ => bail!("préciser --on ou --off"),
    };

    // Same guard as the API: §3 requires an instance admin who signs in without
    // Telegram, and nobody could grant the right back.
    if !on {
        let (admins,): (i64,) =
            sqlx::query_as("SELECT count(*) FROM users WHERE is_instance_admin")
                .fetch_one(db)
                .await?;
        let (target,): (bool,) =
            sqlx::query_as("SELECT is_instance_admin FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_one(db)
                .await?;
        if admins <= 1 && target {
            bail!("c'est le dernier administrateur d'instance — en nommer un autre d'abord");
        }
    }

    sqlx::query("UPDATE users SET is_instance_admin = $2 WHERE id = $1")
        .bind(user_id)
        .bind(on)
        .execute(db)
        .await?;
    println!("instance admin: {}", if on { "granted" } else { "revoked" });
    Ok(())
}

/// Accepts an email or a uuid, so a line copied from `list` always works.
async fn resolve(db: &PgPool, who: &str) -> Result<Uuid> {
    if let Ok(id) = who.parse::<Uuid>() {
        let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE id = $1")
            .bind(id)
            .fetch_optional(db)
            .await?;
        if let Some((id,)) = row {
            return Ok(id);
        }
    }
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM users WHERE lower(email) = lower($1)")
            .bind(who)
            .fetch_optional(db)
            .await?;
    let (id,) = row.with_context(|| format!("utilisateur introuvable : {who}"))?;
    Ok(id)
}

async fn print_link(db: &PgPool, config: &backline::Config, user_id: Uuid) -> Result<()> {
    invitations::expire_pending(db, user_id)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let code = invitations::issue(db, user_id, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{}", invitations::url(&config.public_base_url, &code));
    Ok(())
}
