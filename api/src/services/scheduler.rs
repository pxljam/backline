//! The job processing loop. This is where the behaviours the PRD demands
//! "without intervention" live: logistics reminders, run sheet, publication
//! reminders, going-live alert, render purge.

use crate::error::AppResult;
use crate::services::{jobs, notify, run_sheet};
use crate::state::AppState;
use chrono::{Duration, Utc};
use chrono_tz::Europe::Paris;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn run_forever(state: AppState) {
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(5));
    loop {
        ticker.tick().await;
        if let Err(e) = tick(&state).await {
            tracing::error!(error = %e, "job loop");
        }
    }
}

/// One pass: claims due jobs, runs them, delivers pending notifications.
/// Exposed so tests can trigger it on demand rather than waiting on a clock.
pub async fn tick(state: &AppState) -> AppResult<usize> {
    let claimed = jobs::claim(&state.db, 20).await?;
    let n = claimed.len();
    for job in claimed {
        match dispatch(state, &job).await {
            Ok(()) => jobs::mark_done(&state.db, job.id).await?,
            Err(e) => jobs::mark_failed(&state.db, &job, &e.to_string()).await?,
        }
    }
    notify::deliver_pending(state, 50).await?;
    Ok(n)
}

async fn dispatch(state: &AppState, job: &jobs::Job) -> AppResult<()> {
    let db = &state.db;
    let uuid = |k: &str| -> AppResult<Uuid> {
        job.payload
            .get(k)
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| crate::error::AppError::bad_request(format!("job sans {k}")))
    };

    match job.kind.as_str() {
        "logistics_reminder" => {
            let milestone = job
                .payload
                .get("milestone")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            logistics_reminder(db, uuid("event_id")?, milestone).await
        }
        "tech_rider_missing" => tech_rider_missing(db, uuid("event_id")?).await,
        "run_sheet" => send_run_sheet(state, uuid("event_id")?).await,
        "stream_live_alert" => stream_live_alert(db, uuid("event_id")?).await,
        "event_past" => {
            sqlx::query(
                "UPDATE events SET status = 'past', updated_at = now()
                 WHERE id = $1 AND status = 'confirmed'",
            )
            .bind(uuid("event_id")?)
            .execute(db)
            .await?;
            Ok(())
        }
        "publication_due" => publication_due(db, uuid("task_id")?).await,
        "publication_reminder" => publication_reminder(db, uuid("task_id")?).await,
        "publication_admin_alert" => publication_admin_alert(db, uuid("task_id")?).await,
        "render_unclaimed_alert" => render_unclaimed_alert(db, uuid("render_job_id")?).await,
        "purge_renders" => purge_renders(state).await,
        "generate_visuals" => {
            crate::routes::comms::run_generate_visuals(state, uuid("event_id")?).await
        }
        other => {
            tracing::warn!(kind = other, "unknown job kind, ignored");
            Ok(())
        }
    }
}

/// Targeted reminder about vacant slots (§5.4): only members who have
/// **committed to nothing yet** for this event are woken up.
async fn logistics_reminder(db: &PgPool, event_id: Uuid, milestone: &str) -> AppResult<()> {
    let ev: Option<(Uuid, String, chrono::DateTime<Utc>, String)> =
        sqlx::query_as("SELECT collective_id, title, starts_at, status FROM events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(db)
            .await?;
    let Some((collective_id, title, starts_at, status)) = ev else {
        return Ok(());
    };
    if status == "cancelled" {
        return Ok(());
    }

    let vacant: Vec<(String, i32, i64)> = sqlx::query_as(
        "SELECT s.label, s.quantity, count(a.id)
         FROM logistics_slots s
         LEFT JOIN logistics_assignments a ON a.slot_id = s.id
         WHERE s.event_id = $1
         GROUP BY s.id, s.label, s.quantity
         HAVING count(a.id) < s.quantity
         ORDER BY s.position",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    if vacant.is_empty() {
        return Ok(());
    }

    let targets: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT m.user_id FROM memberships m
         WHERE m.collective_id = $1
           AND NOT EXISTS (
             SELECT 1 FROM logistics_assignments a
             JOIN logistics_slots s ON s.id = a.slot_id
             WHERE s.event_id = $2 AND a.user_id = m.user_id)
           AND NOT EXISTS (
             SELECT 1 FROM participations p
             LEFT JOIN group_members gm ON gm.group_id = p.group_id
             WHERE p.event_id = $2 AND (p.user_id = m.user_id OR gm.user_id = m.user_id))",
    )
    .bind(collective_id)
    .bind(event_id)
    .fetch_all(db)
    .await?;

    let list = vacant
        .iter()
        .map(|(label, qty, taken)| format!("{label} ({}/{qty})", taken))
        .collect::<Vec<_>>()
        .join(", ");
    let when = starts_at.with_timezone(&Paris).format("%d/%m").to_string();

    for (user_id,) in &targets {
        notify::push(
            db,
            notify::Notice {
                user_id: *user_id,
                collective_id: Some(collective_id),
                kind: "logistics_vacant",
                title: format!("{milestone} — postes a pourvoir : {title}"),
                body: format!("Le {when}. Encore libre : {list}"),
                payload: json!({ "event_id": event_id, "milestone": milestone }),
            },
        )
        .await?;
    }

    sqlx::query(
        "INSERT INTO logistics_reminders (event_id, milestone, recipients)
         VALUES ($1, $2, $3) ON CONFLICT (event_id, milestone) DO NOTHING",
    )
    .bind(event_id)
    .bind(milestone)
    .bind(targets.len() as i32)
    .execute(db)
    .await?;

    Ok(())
}

/// A **single** reminder at D-14 to the leads of groups without a tech rider,
/// then nothing more (§12).
async fn tech_rider_missing(db: &PgPool, event_id: Uuid) -> AppResult<()> {
    let rows: Vec<(Uuid, String, Uuid)> = sqlx::query_as(
        "SELECT etr.group_id, g.name, e.collective_id
         FROM event_tech_riders etr
         JOIN groups g ON g.id = etr.group_id
         JOIN events e ON e.id = etr.event_id
         WHERE etr.event_id = $1 AND etr.tech_rider_id IS NULL",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    for (group_id, group_name, collective_id) in rows {
        // The group's leads, or failing that all of its members.
        let mut targets: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT user_id FROM group_members WHERE group_id = $1 AND is_admin = TRUE",
        )
        .bind(group_id)
        .fetch_all(db)
        .await?;
        if targets.is_empty() {
            targets = sqlx::query_as("SELECT user_id FROM group_members WHERE group_id = $1")
                .bind(group_id)
                .fetch_all(db)
                .await?;
        }
        for (user_id,) in targets {
            notify::push(
                db,
                notify::Notice {
                    user_id,
                    collective_id: Some(collective_id),
                    kind: "tech_rider_missing",
                    title: format!("{group_name} : pas de fiche technique"),
                    body: "Un evenement approche. Rien ne bloque, mais le lieu n'aura rien a lire."
                        .into(),
                    payload: json!({ "event_id": event_id, "group_id": group_id }),
                },
            )
            .await?;
        }
    }
    Ok(())
}

async fn send_run_sheet(state: &AppState, event_id: Uuid) -> AppResult<()> {
    let ev: Option<(Uuid, String)> =
        sqlx::query_as("SELECT collective_id, status FROM events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(&state.db)
            .await?;
    let Some((collective_id, status)) = ev else {
        return Ok(());
    };
    if status == "cancelled" {
        return Ok(());
    }

    // Recipients: the line-up and the slot holders. These are exactly the
    // people allowed to see phone numbers (§3).
    let targets: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT u.id FROM users u WHERE u.id IN (
             SELECT COALESCE(p.user_id, gm.user_id) FROM participations p
               LEFT JOIN group_members gm ON gm.group_id = p.group_id
               WHERE p.event_id = $1
             UNION
             SELECT a.user_id FROM logistics_assignments a
               JOIN logistics_slots s ON s.id = a.slot_id WHERE s.event_id = $1
             UNION
             SELECT m.user_id FROM memberships m
               WHERE m.collective_id = $2 AND m.role = 'admin')",
    )
    .bind(event_id)
    .bind(collective_id)
    .fetch_all(&state.db)
    .await?;

    for (user_id,) in targets {
        let scope = crate::scope::CollectiveScope::resolve(
            &state.db,
            crate::scope::Actor {
                user_id,
                is_instance_admin: false,
            },
            collective_id,
        )
        .await?;
        let with_phones = run_sheet::may_see_phones(&state.db, &scope, event_id).await?;
        let sheet = run_sheet::build(&state.db, &scope, event_id, with_phones).await?;
        notify::push(
            &state.db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                kind: "run_sheet",
                title: format!("Demain : {}", sheet.title),
                body: run_sheet::to_text(&sheet),
                payload: json!({ "event_id": event_id }),
            },
        )
        .await?;
    }
    Ok(())
}

/// "On est en ligne" — to every member, 15 minutes ahead, with no
/// intervention (§18).
async fn stream_live_alert(db: &PgPool, event_id: Uuid) -> AppResult<()> {
    let ev: Option<(Uuid, String, String)> =
        sqlx::query_as("SELECT collective_id, title, status FROM events WHERE id = $1")
            .bind(event_id)
            .fetch_optional(db)
            .await?;
    let Some((collective_id, title, status)) = ev else {
        return Ok(());
    };
    if status == "cancelled" {
        return Ok(());
    }

    let platforms: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT platform, url FROM event_stream_platforms WHERE event_id = $1")
            .bind(event_id)
            .fetch_all(db)
            .await?;
    let links = platforms
        .iter()
        .map(|(p, u)| match u {
            Some(u) => format!("{p} : {u}"),
            None => p.clone(),
        })
        .collect::<Vec<_>>()
        .join("\n");

    for user_id in notify::collective_member_ids(db, collective_id).await? {
        notify::push(
            db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                // `stream_live` cuts through quiet hours: you cannot catch up
                // on a live stream the next morning.
                kind: "stream_live",
                title: format!("On passe en live dans 15 min — {title}"),
                body: links.clone(),
                payload: json!({ "event_id": event_id }),
            },
        )
        .await?;
    }

    sqlx::query(
        "INSERT INTO event_streams (event_id, live_alert_sent_at) VALUES ($1, now())
         ON CONFLICT (event_id) DO UPDATE SET live_alert_sent_at = now()",
    )
    .bind(event_id)
    .execute(db)
    .await?;
    Ok(())
}

/// At the appointed time: the visual, the caption and the hashtags go to the
/// owner, ready to paste (§11.2, point 3).
async fn publication_due(db: &PgPool, task_id: Uuid) -> AppResult<()> {
    let t: Option<(
        Uuid,
        String,
        String,
        String,
        String,
        Option<Uuid>,
        Option<Uuid>,
        Uuid,
    )> = sqlx::query_as(
        "SELECT p.id, p.label, p.status, p.caption, p.hashtags,
                    p.assignee_id, p.backup_assignee_id, e.collective_id
             FROM publication_tasks p JOIN events e ON e.id = p.event_id
             WHERE p.id = $1",
    )
    .bind(task_id)
    .fetch_optional(db)
    .await?;
    let Some((_, label, status, caption, hashtags, assignee, backup, collective_id)) = t else {
        return Ok(());
    };
    if status == "published" {
        return Ok(());
    }

    let body = format!("{caption}\n\n{hashtags}").trim().to_string();

    match assignee.or(backup) {
        Some(user_id) => {
            notify::push(
                db,
                notify::Notice {
                    user_id,
                    collective_id: Some(collective_id),
                    kind: "publication_due",
                    title: format!("A publier : {label}"),
                    body,
                    payload: json!({ "task_id": task_id }),
                },
            )
            .await?;
            // No confirmation: reminder at +1 h (§11.2, point 5).
            jobs::enqueue(
                db,
                "publication_reminder",
                Utc::now() + Duration::hours(1),
                json!({ "task_id": task_id }),
                Some(format!("task:{task_id}:reminder")),
            )
            .await?;
        }
        None => {
            // Nobody assigned: the admin must hear about it, not silence.
            for user_id in notify::collective_admin_ids(db, collective_id).await? {
                notify::push(
                    db,
                    notify::Notice {
                        user_id,
                        collective_id: Some(collective_id),
                        kind: "admin_alert",
                        title: format!("Tache de com sans responsable : {label}"),
                        body: "L'heure est passee et personne n'est assigne.".into(),
                        payload: json!({ "task_id": task_id }),
                    },
                )
                .await?;
            }
            jobs::enqueue(
                db,
                "publication_admin_alert",
                Utc::now() + Duration::hours(3),
                json!({ "task_id": task_id }),
                Some(format!("task:{task_id}:admin-alert")),
            )
            .await?;
        }
    }
    Ok(())
}

async fn publication_reminder(db: &PgPool, task_id: Uuid) -> AppResult<()> {
    let t: Option<(String, String, Option<Uuid>, Uuid)> = sqlx::query_as(
        "SELECT p.status, p.label, COALESCE(p.assignee_id, p.backup_assignee_id), e.collective_id
         FROM publication_tasks p JOIN events e ON e.id = p.event_id WHERE p.id = $1",
    )
    .bind(task_id)
    .fetch_optional(db)
    .await?;
    let Some((status, label, assignee, collective_id)) = t else {
        return Ok(());
    };
    if status == "published" {
        return Ok(());
    }

    if let Some(user_id) = assignee {
        notify::push(
            db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                kind: "publication_due",
                title: format!("Toujours pas publie : {label}"),
                body: "Appuie sur « ✅ publie » une fois le post en ligne.".into(),
                payload: json!({ "task_id": task_id }),
            },
        )
        .await?;
    }
    sqlx::query("UPDATE publication_tasks SET reminder_count = reminder_count + 1 WHERE id = $1")
        .bind(task_id)
        .execute(db)
        .await?;

    // …alert the admin at +3 h.
    jobs::enqueue(
        db,
        "publication_admin_alert",
        Utc::now() + Duration::hours(2),
        json!({ "task_id": task_id }),
        Some(format!("task:{task_id}:admin-alert")),
    )
    .await?;
    Ok(())
}

async fn publication_admin_alert(db: &PgPool, task_id: Uuid) -> AppResult<()> {
    let t: Option<(String, String, Uuid)> = sqlx::query_as(
        "SELECT p.status, p.label, e.collective_id
         FROM publication_tasks p JOIN events e ON e.id = p.event_id WHERE p.id = $1",
    )
    .bind(task_id)
    .fetch_optional(db)
    .await?;
    let Some((status, label, collective_id)) = t else {
        return Ok(());
    };
    if status == "published" {
        return Ok(());
    }

    sqlx::query(
        "UPDATE publication_tasks SET status = 'missed', admin_alerted_at = now(), updated_at = now()
         WHERE id = $1 AND status <> 'published'",
    )
    .bind(task_id)
    .execute(db)
    .await?;

    for user_id in notify::collective_admin_ids(db, collective_id).await? {
        notify::push(
            db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                kind: "admin_alert",
                title: format!("Publication non confirmee : {label}"),
                body: "Trois heures sans confirmation. La tache est marquee ratee.".into(),
                payload: json!({ "task_id": task_id }),
            },
        )
        .await?;
    }
    Ok(())
}

/// An unclaimed job alerts the admin rather than waiting silently (§10.3).
async fn render_unclaimed_alert(db: &PgPool, render_job_id: Uuid) -> AppResult<()> {
    let j: Option<(String, Uuid, Option<String>)> = sqlx::query_as(
        "SELECT r.status, r.collective_id, c.name
         FROM render_jobs r LEFT JOIN video_compositions c ON c.id = r.composition_id
         WHERE r.id = $1",
    )
    .bind(render_job_id)
    .fetch_optional(db)
    .await?;
    let Some((status, collective_id, name)) = j else {
        return Ok(());
    };
    if status != "queued" {
        return Ok(());
    }

    let machines: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM render_machines
         WHERE revoked_at IS NULL AND last_seen_at > now() - interval '10 minutes'",
    )
    .fetch_one(db)
    .await?;

    for user_id in notify::collective_admin_ids(db, collective_id).await? {
        notify::push(
            db,
            notify::Notice {
                user_id,
                collective_id: Some(collective_id),
                kind: "admin_alert",
                title: format!(
                    "Rendu video non reclame : {}",
                    name.clone().unwrap_or_else(|| "composition".into())
                ),
                body: format!(
                    "{} machine(s) connectee(s). La tache de com reste livrable avec son visuel fixe.",
                    machines.0
                ),
                payload: json!({ "render_job_id": render_job_id }),
            },
        )
        .await?;
    }

    sqlx::query("UPDATE render_jobs SET unclaimed_alert_sent_at = now() WHERE id = $1")
        .bind(render_job_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Purges rendered visuals older than six months: they regenerate identically
/// from their description. Uploaded media is **never** purged automatically
/// (§15).
pub async fn purge_renders(state: &AppState) -> AppResult<()> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, storage_key FROM assets
         WHERE kind = 'render' AND purge_after IS NOT NULL AND purge_after < now()
         LIMIT 500",
    )
    .fetch_all(&state.db)
    .await?;

    for (id, key) in rows {
        if let Err(e) = state.storage.delete(&key).await {
            tracing::warn!(error = %e, key, "purge: object deletion");
        }
        sqlx::query("DELETE FROM assets WHERE id = $1")
            .bind(id)
            .execute(&state.db)
            .await?;
    }

    jobs::enqueue(
        &state.db,
        "purge_renders",
        Utc::now() + Duration::hours(24),
        json!({}),
        Some(format!("purge:{}", Utc::now().date_naive())),
    )
    .await?;
    Ok(())
}
