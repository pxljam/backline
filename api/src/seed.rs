//! Seed data (§16).
//!
//! The set installed by `docker compose up` on a clean machine, and the one the
//! end-to-end tests use. Idempotent: re-running the seed on an already-seeded
//! database duplicates nothing.

use crate::error::AppResult;
use crate::services::{comms, events as events_svc, ical, provisioning};
use anyhow::Result;
use chrono::{Duration, NaiveTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn run(db: &PgPool) -> Result<()> {
    if already_seeded(db).await? {
        tracing::info!("seed data already present");
        return Ok(());
    }
    seed(db).await.map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

async fn already_seeded(db: &PgPool) -> Result<bool> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM collectives WHERE slug = 'bonsoir-techno'")
            .fetch_one(db)
            .await?;
    Ok(n > 0)
}

/// Creates a user, or finds them if they already exist.
async fn upsert_user(
    db: &PgPool,
    display_name: &str,
    stage_name: Option<&str>,
    phone: &str,
    email: Option<&str>,
) -> AppResult<Uuid> {
    if let Some(email) = email {
        if let Some((id,)) =
            sqlx::query_as::<_, (Uuid,)>("SELECT id FROM users WHERE lower(email) = lower($1)")
                .bind(email)
                .fetch_optional(db)
                .await?
        {
            return Ok(id);
        }
    }
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO users (display_name, stage_name, phone, email) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(display_name)
    .bind(stage_name)
    .bind(phone)
    .bind(email)
    .fetch_one(db)
    .await?;
    ical::ensure_token(db, "user", id).await?;
    Ok(id)
}

async fn add_member(db: &PgPool, collective_id: Uuid, user_id: Uuid, role: &str) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO memberships (collective_id, user_id, role) VALUES ($1, $2, $3)
         ON CONFLICT (collective_id, user_id) DO UPDATE SET role = EXCLUDED.role",
    )
    .bind(collective_id)
    .bind(user_id)
    .bind(role)
    .execute(db)
    .await?;
    Ok(())
}

async fn add_group(
    db: &PgPool,
    collective_id: Uuid,
    slug: &str,
    name: &str,
    description: &str,
) -> AppResult<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO groups (collective_id, slug, name, description) VALUES ($1, $2, $3, $4)
         RETURNING id",
    )
    .bind(collective_id)
    .bind(slug)
    .bind(name)
    .bind(description)
    .fetch_one(db)
    .await?;
    provisioning::seed_brand(db, collective_id, name, Some(id)).await?;
    ical::ensure_token(db, "group", id).await?;
    Ok(id)
}

async fn add_group_member(
    db: &PgPool,
    group_id: Uuid,
    user_id: Uuid,
    role_label: &str,
    is_admin: bool,
) -> AppResult<()> {
    sqlx::query(
        "INSERT INTO group_members (group_id, user_id, role_label, is_admin)
         VALUES ($1, $2, $3, $4) ON CONFLICT DO NOTHING",
    )
    .bind(group_id)
    .bind(user_id)
    .bind(role_label)
    .bind(is_admin)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn seed(db: &PgPool) -> AppResult<()> {
    provisioning::ensure_format_catalog(db).await?;

    // --- Instance admin ----------------------------------------------------
    // At least one instance admin must be able to get in without Telegram (§3).
    let root = upsert_user(db, "Admin instance", None, "", Some("admin@backline.local")).await?;
    let hash = crate::auth::hash_password("backline-admin").map_err(crate::AppError::Internal)?;
    sqlx::query("UPDATE users SET password_hash = $2, is_instance_admin = TRUE WHERE id = $1")
        .bind(root)
        .bind(hash)
        .execute(db)
        .await?;

    // --- Collective --------------------------------------------------------
    let collective =
        provisioning::create_collective(db, "bonsoir-techno", "Bonsoir Techno").await?;

    // Antoine creates the collective and is its admin. They belong to no
    // group — §16's assumption, to be confirmed.
    let antoine = upsert_user(
        db,
        "Antoine",
        None,
        "+33600000001",
        Some("antoine@bonsoirtechno.fr"),
    )
    .await?;
    let anas = upsert_user(
        db,
        "Anas",
        Some("Anas"),
        "+33600000002",
        Some("anas@ramas.fr"),
    )
    .await?;
    let romain = upsert_user(
        db,
        "Romain",
        Some("Romain"),
        "+33600000003",
        Some("romain@ramas.fr"),
    )
    .await?;
    let mathieu = upsert_user(
        db,
        "Mathieu",
        Some("Dante3p"),
        "+33600000004",
        Some("mathieu@dante3p.fr"),
    )
    .await?;

    add_member(db, collective, antoine, "admin").await?;
    for u in [anas, romain, mathieu] {
        add_member(db, collective, u, "member").await?;
    }
    // The instance admin is also a member: otherwise they would see nothing
    // from the collective's interface.
    add_member(db, collective, root, "admin").await?;

    // Reference group for the tests: Ramas, a duo whose two members are also
    // members of the collective.
    let ramas = add_group(db, collective, "ramas", "Ramas", "Duo live techno").await?;
    add_group_member(db, ramas, anas, "MAO / synthes", true).await?;
    add_group_member(db, ramas, romain, "machines / live", true).await?;

    let dante3p = add_group(db, collective, "dante3p", "Dante3p", "Projet solo").await?;
    add_group_member(db, dante3p, mathieu, "DJ", true).await?;

    // --- Social accounts: who has access, never the passwords (§11.3) ------
    for (platform, handle) in [
        ("instagram", "@ramas"),
        ("tiktok", "@ramas"),
        ("youtube", "Ramas"),
    ] {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO social_accounts (collective_id, group_id, platform, handle, url, mode, notes)
             VALUES ($1, $2, $3, $4, $5, 'shared', $6) RETURNING id",
        )
        .bind(collective)
        .bind(ramas)
        .bind(platform)
        .bind(handle)
        .bind(format!("https://{platform}.com/ramas"))
        .bind("Mot de passe partage entre Anas et Romain, dans le gestionnaire externe.")
        .fetch_one(db)
        .await?;
        for u in [anas, romain] {
            sqlx::query(
                "INSERT INTO social_account_access (social_account_id, user_id) VALUES ($1, $2)
                 ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(u)
            .execute(db)
            .await?;
        }
    }

    for (platform, handle) in [
        ("instagram", "@bonsoirtechno"),
        ("tiktok", "@bonsoirtechno"),
    ] {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO social_accounts (collective_id, group_id, platform, handle, url, mode, notes)
             VALUES ($1, NULL, $2, $3, $4, 'shared', $5) RETURNING id",
        )
        .bind(collective)
        .bind(platform)
        .bind(handle)
        .bind(format!("https://{platform}.com/bonsoirtechno"))
        .bind("Detenu par Antoine.")
        .fetch_one(db)
        .await?;
        sqlx::query(
            "INSERT INTO social_account_access (social_account_id, user_id) VALUES ($1, $2)",
        )
        .bind(id)
        .bind(antoine)
        .execute(db)
        .await?;
    }

    // --- Ramas's tech rider, published -------------------------------------
    sqlx::query(
        "INSERT INTO tech_riders (group_id, version, status, data, created_by)
         VALUES ($1, 1, 'published', $2, $3)",
    )
    .bind(ramas)
    .bind(json!({
        "identity": { "style": "live techno hybride", "set_duration_min": 60, "stage_headcount": 2 },
        "stage_lineup": [
            { "member": "Anas", "instrument": "MAO / synthes", "position": "jardin" },
            { "member": "Romain", "instrument": "machines / drum", "position": "cour" }
        ],
        "input_list": [
            { "channel": 1, "source": "Master L", "mic": "DI", "insert": "", "stand": "" },
            { "channel": 2, "source": "Master R", "mic": "DI", "insert": "", "stand": "" },
            { "channel": 3, "source": "Voix", "mic": "SM58", "insert": "", "stand": "perche" }
        ],
        "backline": {
            "brought": "2 controleurs, 1 synthe, 1 interface audio",
            "requested": "2 tables a hauteur reglable, 4 prises 230V par poste"
        },
        "sound": { "monitors": "2 wedges", "circuits": "2 circuits independants", "foh": "systeme adapte a la jauge" },
        "light": "Ambiance sombre, contres froids, pas de poursuite.",
        "hospitality": "Loge fermant a cle, eau, 4 repas vegetariens.",
        "arrival": { "load_in": "17:00", "soundcheck": "18:30" },
        "contacts": [ { "name": "Romain", "role": "technique", "phone": "+33600000003" } ]
    }))
    .bind(romain)
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO press_kits (group_id, bio_short, bio_long, links)
         VALUES ($1, $2, $3, $4) ON CONFLICT (group_id) DO NOTHING",
    )
    .bind(ramas)
    .bind("Duo live techno forme a Lyon.")
    .bind("Ramas est un duo live techno : machines, synthes et textures, pensees pour le club comme pour la salle.")
    .bind(json!([
        { "label": "Bandcamp", "url": "https://ramas.bandcamp.com" },
        { "label": "SoundCloud", "url": "https://soundcloud.com/ramas" }
    ]))
    .execute(db)
    .await?;

    // --- Venue + contact ---------------------------------------------------
    let (venue,): (Uuid,) = sqlx::query_as(
        "INSERT INTO venues (collective_id, name, address, city, country, capacity, notes)
         VALUES ($1, 'Le Sonic', '4 quai des Etroits', 'Lyon', 'France', 250, 'Peniche, chargement par le quai.')
         RETURNING id",
    )
    .bind(collective)
    .fetch_one(db)
    .await?;
    let (contact,): (Uuid,) = sqlx::query_as(
        "INSERT INTO venue_contacts (venue_id, name, role, phone, email)
         VALUES ($1, 'Claire', 'programmation', '+33600000010', 'claire@lesonic.fr') RETURNING id",
    )
    .bind(venue)
    .fetch_one(db)
    .await?;

    // --- Opportunity with three dates, poll partially filled in ------------
    let (opportunity,): (Uuid,) = sqlx::query_as(
        "INSERT INTO opportunities
            (collective_id, venue_id, venue_contact_id, title, conditions, status, created_by)
         VALUES ($1, $2, $3, 'Soiree Bonsoir Techno au Sonic', 'Trois dates proposees par le lieu. Backline a confirmer.', 'poll_open', $4)
         RETURNING id",
    )
    .bind(collective)
    .bind(venue)
    .bind(contact)
    .bind(antoine)
    .fetch_one(db)
    .await?;
    sqlx::query("INSERT INTO opportunity_hosts (opportunity_id, group_id) VALUES ($1, $2)")
        .bind(opportunity)
        .bind(ramas)
        .execute(db)
        .await?;

    let today = Utc::now().date_naive();
    let mut candidate_ids = Vec::new();
    for (i, offset) in [45i64, 52, 59].iter().enumerate() {
        let (id,): (Uuid,) = sqlx::query_as(
            "INSERT INTO candidate_dates (opportunity_id, day, start_time, end_time, notes, position)
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(opportunity)
        .bind(today + Duration::days(*offset))
        .bind(NaiveTime::from_hms_opt(22, 0, 0))
        .bind(NaiveTime::from_hms_opt(3, 0, 0))
        .bind(if i == 1 { Some("Le lieu prefere cette date.") } else { None })
        .bind(i as i32)
        .fetch_one(db)
        .await?;
        candidate_ids.push(id);
    }

    let (poll,): (Uuid,) = sqlx::query_as(
        "INSERT INTO availability_polls (opportunity_id, opened_by) VALUES ($1, $2) RETURNING id",
    )
    .bind(opportunity)
    .bind(antoine)
    .fetch_one(db)
    .await?;

    // The poll is **partially** filled in: Mathieu has not answered yet, and
    // that is exactly the state the matrix must be able to show.
    let answers: [(Uuid, [(&str, bool); 3]); 3] = [
        (anas, [("yes", true), ("yes", true), ("no", false)]),
        (romain, [("maybe", false), ("yes", true), ("no", false)]),
        (antoine, [("yes", false), ("yes", false), ("yes", false)]),
    ];
    for (user, per_date) in answers {
        for (i, (status, wants)) in per_date.iter().enumerate() {
            sqlx::query(
                "INSERT INTO availabilities (poll_id, candidate_date_id, user_id, status, wants_to_play)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(poll)
            .bind(candidate_ids[i])
            .bind(user)
            .bind(status)
            .bind(wants)
            .execute(db)
            .await?;
        }
    }

    // --- One fully confirmed event -----------------------------------------
    let concert = make_event(
        db,
        collective,
        "concert",
        "Bonsoir Techno #12 — Ramas live",
        Utc::now() + Duration::days(21),
        Some(Utc::now() + Duration::days(21) + Duration::hours(5)),
        Some(venue),
        None,
    )
    .await?;
    sqlx::query(
        "UPDATE events SET doors_at = $2, soundcheck_at = $3, set_times_state = 'defined',
                           set_times_public = TRUE, capacity = 250 WHERE id = $1",
    )
    .bind(concert)
    .bind(Utc::now() + Duration::days(21) - Duration::hours(1))
    .bind(Utc::now() + Duration::days(21) - Duration::hours(3))
    .execute(db)
    .await?;

    sqlx::query(
        "INSERT INTO participations (event_id, group_id, status, stage_role, slot_start, slot_end, position)
         VALUES ($1, $2, 'confirmed', 'live', $3, $4, 0)",
    )
    .bind(concert)
    .bind(ramas)
    .bind(NaiveTime::from_hms_opt(23, 0, 0))
    .bind(NaiveTime::from_hms_opt(0, 15, 0))
    .execute(db)
    .await?;
    sqlx::query(
        "INSERT INTO participations (event_id, group_id, status, stage_role, slot_start, slot_end, position)
         VALUES ($1, $2, 'confirmed', 'dj set', $3, $4, 1)",
    )
    .bind(concert)
    .bind(dante3p)
    .bind(NaiveTime::from_hms_opt(0, 30, 0))
    .bind(NaiveTime::from_hms_opt(2, 0, 0))
    .execute(db)
    .await?;

    // A few slots already taken, others deliberately vacant: that is what
    // makes the reminders observable.
    let slots: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT id, label FROM logistics_slots WHERE event_id = $1 ORDER BY position",
    )
    .bind(concert)
    .fetch_all(db)
    .await?;
    for (slot_id, label) in &slots {
        if label == "son" {
            sqlx::query("INSERT INTO logistics_assignments (slot_id, user_id) VALUES ($1, $2)")
                .bind(slot_id)
                .bind(romain)
                .execute(db)
                .await?;
        }
        if label == "photo" {
            sqlx::query("INSERT INTO logistics_assignments (slot_id, user_id) VALUES ($1, $2)")
                .bind(slot_id)
                .bind(antoine)
                .execute(db)
                .await?;
        }
    }

    events_svc::attach_tech_riders(db, concert).await?;
    assign_publication_tasks(db, collective, concert).await?;

    // --- One residency -----------------------------------------------------
    let residency = make_event(
        db,
        collective,
        "residency",
        "Residence Ramas — studio de la Croix-Rousse",
        Utc::now() + Duration::days(60),
        Some(Utc::now() + Duration::days(64)),
        None,
        Some(ramas),
    )
    .await?;
    sqlx::query(
        "INSERT INTO participations (event_id, group_id, status, position) VALUES ($1, $2, 'confirmed', 0)",
    )
    .bind(residency)
    .bind(ramas)
    .execute(db)
    .await?;
    // "Je viens", with no further detail: the expected default answer (§6).
    for (user, answer) in [(anas, "coming"), (romain, "coming"), (mathieu, "unsure")] {
        let (pid,): (Uuid,) = sqlx::query_as(
            "INSERT INTO residency_presences (event_id, user_id, answer) VALUES ($1, $2, $3)
             RETURNING id",
        )
        .bind(residency)
        .bind(user)
        .bind(answer)
        .fetch_one(db)
        .await?;
        // Romain ticks two days: a convenience for the admin, never a requirement.
        if user == romain {
            for d in 0..2 {
                sqlx::query(
                    "INSERT INTO residency_presence_days (presence_id, day) VALUES ($1, $2)",
                )
                .bind(pid)
                .bind((Utc::now() + Duration::days(60 + d)).date_naive())
                .execute(db)
                .await?;
            }
        }
    }

    // --- One stream --------------------------------------------------------
    let stream = make_event(
        db,
        collective,
        "stream",
        "Bonsoir Techno Live #4",
        Utc::now() + Duration::days(10),
        Some(Utc::now() + Duration::days(10) + Duration::hours(2)),
        None,
        None,
    )
    .await?;
    sqlx::query(
        "INSERT INTO event_streams (event_id, capture_location, planned_duration_min)
         VALUES ($1, 'Studio de la Croix-Rousse', 120)",
    )
    .bind(stream)
    .execute(db)
    .await?;
    for (platform, url) in [
        ("twitch", "https://twitch.tv/bonsoirtechno"),
        ("youtube", "https://youtube.com/@bonsoirtechno/live"),
    ] {
        sqlx::query(
            "INSERT INTO event_stream_platforms (event_id, platform, url) VALUES ($1, $2, $3)",
        )
        .bind(stream)
        .bind(platform)
        .bind(url)
        .execute(db)
        .await?;
    }
    sqlx::query(
        "INSERT INTO participations (event_id, group_id, status, stage_role, slot_start, slot_end, position)
         VALUES ($1, $2, 'confirmed', 'live', $3, $4, 0)",
    )
    .bind(stream)
    .bind(ramas)
    .bind(NaiveTime::from_hms_opt(21, 0, 0))
    .bind(NaiveTime::from_hms_opt(22, 0, 0))
    .execute(db)
    .await?;
    assign_publication_tasks(db, collective, stream).await?;

    // --- One announcement template, derived across three formats -----------
    crate::services::templates::seed_default_template(db, collective, None).await?;

    tracing::info!("seed data installed: Bonsoir Techno, Ramas, Dante3p");
    Ok(())
}

/// A demo event, with its logistics slots, its comms plan and its scheduled
/// jobs — exactly the path a confirmation takes.
#[allow(clippy::too_many_arguments)]
async fn make_event(
    db: &PgPool,
    collective: Uuid,
    type_key: &str,
    title: &str,
    starts_at: chrono::DateTime<Utc>,
    ends_at: Option<chrono::DateTime<Utc>>,
    venue: Option<Uuid>,
    host_group: Option<Uuid>,
) -> AppResult<Uuid> {
    let (type_id,): (Uuid,) =
        sqlx::query_as("SELECT id FROM event_types WHERE collective_id = $1 AND key = $2")
            .bind(collective)
            .bind(type_key)
            .fetch_one(db)
            .await?;

    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO events
            (collective_id, event_type_id, venue_id, host_group_id, title, status, starts_at, ends_at)
         VALUES ($1, $2, $3, $4, $5, 'confirmed', $6, $7) RETURNING id",
    )
    .bind(collective)
    .bind(type_id)
    .bind(venue)
    .bind(host_group)
    .bind(title)
    .bind(starts_at)
    .bind(ends_at)
    .fetch_one(db)
    .await?;

    for (i, (label, qty)) in comms::default_logistics(type_key).iter().enumerate() {
        sqlx::query(
            "INSERT INTO logistics_slots (event_id, label, quantity, position) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(label)
        .bind(qty)
        .bind(i as i32)
        .execute(db)
        .await?;
    }

    comms::instantiate_plan(db, id).await?;
    events_svc::schedule_event_jobs(db, id).await?;
    Ok(id)
}

/// Attaches every comms task to a social account and to an owner who has
/// access to it — §11.2's invariant holds for the demo data too.
async fn assign_publication_tasks(db: &PgPool, collective: Uuid, event_id: Uuid) -> AppResult<()> {
    let account: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM social_accounts
         WHERE collective_id = $1 AND group_id IS NULL AND platform = 'instagram' LIMIT 1",
    )
    .bind(collective)
    .fetch_optional(db)
    .await?;
    let Some((account_id,)) = account else {
        return Ok(());
    };

    let holder: Option<(Uuid,)> = sqlx::query_as(
        "SELECT user_id FROM social_account_access WHERE social_account_id = $1 LIMIT 1",
    )
    .bind(account_id)
    .fetch_optional(db)
    .await?;
    let Some((assignee,)) = holder else {
        return Ok(());
    };

    sqlx::query(
        "UPDATE publication_tasks
         SET social_account_id = $2, assignee_id = $3, status = 'assigned', updated_at = now()
         WHERE event_id = $1 AND status = 'draft'",
    )
    .bind(event_id)
    .bind(account_id)
    .bind(assignee)
    .execute(db)
    .await?;
    Ok(())
}
