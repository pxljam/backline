//! Conversion d'une opportunite en evenement, et tout ce que la confirmation
//! declenche (§5.3). Un seul endroit : sinon un evenement cree autrement
//! arriverait sans plan de com ni relances.

use crate::error::{AppError, AppResult};
use crate::scope::CollectiveScope;
use crate::services::{comms, jobs, notify};
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::Paris;
use serde::Deserialize;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct LineUpEntry {
    #[serde(default)]
    pub group_id: Option<Uuid>,
    #[serde(default)]
    pub user_id: Option<Uuid>,
    #[serde(default)]
    pub stage_role: Option<String>,
    #[serde(default)]
    pub slot_start: Option<NaiveTime>,
    #[serde(default)]
    pub slot_end: Option<NaiveTime>,
}

#[derive(Debug, Deserialize)]
pub struct ConvertRequest {
    pub candidate_date_id: Uuid,
    pub event_type_key: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub line_up: Vec<LineUpEntry>,
    #[serde(default)]
    pub host_group_id: Option<Uuid>,
    #[serde(default)]
    pub ends_at: Option<DateTime<Utc>>,
}

/// Combine une date candidate et son horaire en un instant UTC. Sans horaire,
/// on retient 20h00 locales : l'heure d'un concert par defaut, corrigeable.
pub fn local_datetime(day: NaiveDate, time: Option<NaiveTime>) -> DateTime<Utc> {
    let t = time.unwrap_or_else(|| NaiveTime::from_hms_opt(20, 0, 0).unwrap());
    let naive = day.and_time(t);
    Paris
        .from_local_datetime(&naive)
        .earliest()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc.from_utc_datetime(&naive))
}

pub async fn convert(
    db: &PgPool,
    scope: &CollectiveScope,
    opportunity_id: Uuid,
    req: ConvertRequest,
) -> AppResult<Uuid> {
    scope.require_admin()?;

    let opp: Option<(Uuid, Option<Uuid>, String, String)> = sqlx::query_as(
        "SELECT id, venue_id, title, status FROM opportunities
         WHERE id = $1 AND collective_id = $2",
    )
    .bind(opportunity_id)
    .bind(scope.collective_id())
    .fetch_optional(db)
    .await?;
    let (_, venue_id, opp_title, status) =
        opp.ok_or_else(|| AppError::not_found("opportunite introuvable"))?;

    if status == "confirmed" {
        return Err(AppError::conflict("cette opportunite est deja confirmee"));
    }

    let date: Option<(NaiveDate, Option<NaiveTime>, Option<NaiveTime>)> = sqlx::query_as(
        "SELECT day, start_time, end_time FROM candidate_dates
         WHERE id = $1 AND opportunity_id = $2",
    )
    .bind(req.candidate_date_id)
    .bind(opportunity_id)
    .fetch_optional(db)
    .await?;
    let (day, start_time, end_time) =
        date.ok_or_else(|| AppError::not_found("date candidate introuvable"))?;

    let event_type: Option<(Uuid, bool)> = sqlx::query_as(
        "SELECT id, is_range FROM event_types WHERE collective_id = $1 AND key = $2",
    )
    .bind(scope.collective_id())
    .bind(&req.event_type_key)
    .fetch_optional(db)
    .await?;
    let (event_type_id, _is_range) =
        event_type.ok_or_else(|| AppError::not_found("type d'evenement inconnu"))?;

    if let Some(gid) = req.host_group_id {
        scope.check_group(db, gid).await?;
    }

    let starts_at = local_datetime(day, start_time);
    let ends_at = req
        .ends_at
        .or_else(|| end_time.map(|t| local_datetime(day, Some(t))));

    let mut tx = db.begin().await?;

    let (event_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO events
            (collective_id, event_type_id, opportunity_id, venue_id, host_group_id,
             title, status, starts_at, ends_at)
         VALUES ($1, $2, $3, $4, $5, $6, 'confirmed', $7, $8)
         RETURNING id",
    )
    .bind(scope.collective_id())
    .bind(event_type_id)
    .bind(opportunity_id)
    .bind(venue_id)
    .bind(req.host_group_id)
    .bind(req.title.unwrap_or(opp_title))
    .bind(starts_at)
    .bind(ends_at)
    .fetch_one(&mut *tx)
    .await?;

    for (i, entry) in req.line_up.iter().enumerate() {
        if entry.group_id.is_none() && entry.user_id.is_none() {
            return Err(AppError::bad_request(
                "chaque ligne du line-up vise un groupe ou une personne",
            ));
        }
        sqlx::query(
            "INSERT INTO participations
                (event_id, group_id, user_id, status, stage_role, slot_start, slot_end, position)
             VALUES ($1, $2, $3, 'confirmed', $4, $5, $6, $7)",
        )
        .bind(event_id)
        .bind(entry.group_id)
        .bind(entry.user_id)
        .bind(&entry.stage_role)
        .bind(entry.slot_start)
        .bind(entry.slot_end)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }

    // 3. Postes logistiques, crees **vides** : personne n'est assigne d'office.
    for (i, (label, qty)) in comms::default_logistics(&req.event_type_key)
        .iter()
        .enumerate()
    {
        sqlx::query(
            "INSERT INTO logistics_slots (event_id, label, quantity, position)
             VALUES ($1, $2, $3, $4)",
        )
        .bind(event_id)
        .bind(label)
        .bind(qty)
        .bind(i as i32)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("UPDATE opportunities SET status = 'confirmed', updated_at = now() WHERE id = $1")
        .bind(opportunity_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE availability_polls SET closed_at = now() WHERE opportunity_id = $1 AND closed_at IS NULL")
        .bind(opportunity_id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // 4. Plan de com instancie depuis le type d'evenement.
    comms::instantiate_plan(db, event_id).await?;
    // 5. Fiches techniques des groupes du line-up, rattachees a leur version courante.
    attach_tech_riders(db, event_id).await?;
    // 1. Le calendrier et les flux iCal se mettent a jour d'eux-memes : ils
    //    lisent `events`. Rien a pousser.
    schedule_event_jobs(db, event_id).await?;
    notify_lineup(db, scope, opportunity_id, event_id).await?;

    Ok(event_id)
}

/// Rattache, pour chaque groupe du line-up, la derniere fiche technique
/// publiee. Aucune fiche = ligne quand meme creee, avec `NULL` : l'absence est
/// **signalee, jamais bloquante** (§12).
pub async fn attach_tech_riders(db: &PgPool, event_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO event_tech_riders (event_id, group_id, tech_rider_id)
         SELECT p.event_id, p.group_id,
                (SELECT r.id FROM tech_riders r
                  WHERE r.group_id = p.group_id AND r.status = 'published'
                  ORDER BY r.version DESC LIMIT 1)
         FROM participations p
         WHERE p.event_id = $1 AND p.group_id IS NOT NULL
         ON CONFLICT (event_id, group_id) DO NOTHING",
    )
    .bind(event_id)
    .execute(db)
    .await?;
    Ok(())
}

/// Programme tout ce qui doit partir tout seul : relances logistiques,
/// feuille de route, taches de publication, alerte de mise en ligne, passage
/// au statut `passe`.
pub async fn schedule_event_jobs(db: &PgPool, event_id: Uuid) -> AppResult<()> {
    let (starts_at, ends_at, type_key): (DateTime<Utc>, Option<DateTime<Utc>>, String) =
        sqlx::query_as(
            "SELECT e.starts_at, e.ends_at, t.key FROM events e
             JOIN event_types t ON t.id = e.event_type_id WHERE e.id = $1",
        )
        .bind(event_id)
        .fetch_one(db)
        .await?;

    // Relances sur les postes vacants a J-14, J-7, J-2 (§5.4).
    for (milestone, days) in [("j-14", 14), ("j-7", 7), ("j-2", 2)] {
        jobs::enqueue(
            db,
            "logistics_reminder",
            starts_at - Duration::days(days),
            json!({ "event_id": event_id, "milestone": milestone }),
            Some(format!("event:{event_id}:logistics:{milestone}")),
        )
        .await?;
    }

    // Rappel unique au referent du groupe sans fiche technique, a J-14. Puis
    // plus rien — pas de harcelement (§12).
    jobs::enqueue(
        db,
        "tech_rider_missing",
        starts_at - Duration::days(14),
        json!({ "event_id": event_id }),
        Some(format!("event:{event_id}:rider-missing")),
    )
    .await?;

    // Feuille de route envoyee la veille (§5.5).
    jobs::enqueue(
        db,
        "run_sheet",
        starts_at - Duration::days(1),
        json!({ "event_id": event_id }),
        Some(format!("event:{event_id}:run-sheet")),
    )
    .await?;

    // Un stream previent tous les membres 15 minutes avant, sans intervention (§18).
    if type_key == "stream" {
        jobs::enqueue(
            db,
            "stream_live_alert",
            starts_at - Duration::minutes(15),
            json!({ "event_id": event_id }),
            Some(format!("event:{event_id}:live-alert")),
        )
        .await?;
    }

    jobs::enqueue(
        db,
        "event_past",
        ends_at.unwrap_or(starts_at) + Duration::hours(6),
        json!({ "event_id": event_id }),
        Some(format!("event:{event_id}:past")),
    )
    .await?;

    schedule_publication_jobs(db, event_id).await?;
    Ok(())
}

pub async fn schedule_publication_jobs(db: &PgPool, event_id: Uuid) -> AppResult<()> {
    let tasks: Vec<(Uuid, DateTime<Utc>)> =
        sqlx::query_as("SELECT id, scheduled_at FROM publication_tasks WHERE event_id = $1")
            .bind(event_id)
            .fetch_all(db)
            .await?;

    for (task_id, at) in tasks {
        jobs::enqueue(
            db,
            "publication_due",
            at,
            json!({ "task_id": task_id }),
            Some(format!("task:{task_id}:due")),
        )
        .await?;
    }
    Ok(())
}

/// Notifie les retenus **et les non-retenus** (§5.3, point 2). Ne rien dire aux
/// non-retenus est la principale source de rancune dans un collectif.
async fn notify_lineup(
    db: &PgPool,
    scope: &CollectiveScope,
    opportunity_id: Uuid,
    event_id: Uuid,
) -> AppResult<()> {
    let (title, starts_at): (String, DateTime<Utc>) =
        sqlx::query_as("SELECT title, starts_at FROM events WHERE id = $1")
            .bind(event_id)
            .fetch_one(db)
            .await?;
    let when = starts_at
        .with_timezone(&Paris)
        .format("%d/%m/%Y a %H:%M")
        .to_string();

    let retained: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT u.id FROM participations p
         LEFT JOIN group_members gm ON gm.group_id = p.group_id
         JOIN users u ON u.id = COALESCE(p.user_id, gm.user_id)
         WHERE p.event_id = $1",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;
    let retained: Vec<Uuid> = retained.into_iter().map(|(id,)| id).collect();

    for user_id in &retained {
        notify::push(
            db,
            notify::Notice {
                user_id: *user_id,
                collective_id: Some(scope.collective_id()),
                kind: "lineup_retained",
                title: format!("Tu joues : {title}"),
                body: format!("Le {when}. Merci de confirmer ta presence."),
                payload: json!({ "event_id": event_id }),
            },
        )
        .await?;
    }

    // Les volontaires du sondage qui ne figurent pas au line-up.
    let volunteers: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT DISTINCT a.user_id FROM availabilities a
         JOIN candidate_dates d ON d.id = a.candidate_date_id
         WHERE d.opportunity_id = $1 AND a.wants_to_play = TRUE",
    )
    .bind(opportunity_id)
    .fetch_all(db)
    .await?;

    for (user_id,) in volunteers {
        if retained.contains(&user_id) {
            continue;
        }
        notify::push(
            db,
            notify::Notice {
                user_id,
                collective_id: Some(scope.collective_id()),
                kind: "lineup_not_retained",
                title: format!("Line-up arrete : {title}"),
                body: format!(
                    "Le {when}. Tu n'es pas sur ce line-up cette fois — la date est calee, \
                     les postes logistiques sont ouverts si tu veux en prendre un."
                ),
                payload: json!({ "event_id": event_id }),
            },
        )
        .await?;
    }

    Ok(())
}
