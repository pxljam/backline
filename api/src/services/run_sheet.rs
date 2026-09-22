//! Feuille de route du jour J (§5.5).
//!
//! C'est le seul endroit ou les telephones apparaissent : la visibilite est
//! donc restreinte aux admins du collectif et aux membres de l'evenement (§3).

use crate::error::{AppError, AppResult};
use crate::scope::CollectiveScope;
use chrono::{DateTime, Utc};
use chrono_tz::Europe::Paris;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct RunSheet {
    pub event_id: Uuid,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub doors_at: Option<DateTime<Utc>>,
    pub soundcheck_at: Option<DateTime<Utc>>,
    pub venue: Option<Venue>,
    pub line_up: Vec<LineUpLine>,
    pub logistics: Vec<LogisticsLine>,
    pub tech_riders: Vec<RiderLine>,
}

#[derive(Debug, Serialize)]
pub struct Venue {
    pub name: String,
    pub address: Option<String>,
    pub city: Option<String>,
    pub contacts: Vec<Contact>,
}

#[derive(Debug, Serialize)]
pub struct Contact {
    pub name: String,
    pub role: Option<String>,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LineUpLine {
    pub label: String,
    pub stage_role: Option<String>,
    pub slot: Option<String>,
    pub members: Vec<Person>,
}

#[derive(Debug, Serialize)]
pub struct Person {
    pub user_id: Uuid,
    pub name: String,
    pub phone: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct LogisticsLine {
    pub label: String,
    pub quantity: i32,
    pub assignees: Vec<Person>,
    pub vacant: i32,
}

#[derive(Debug, Serialize)]
pub struct RiderLine {
    pub group_id: Uuid,
    pub group_name: String,
    pub tech_rider_id: Option<Uuid>,
    pub version: Option<i32>,
}

/// L'utilisateur voit-il les telephones de cette feuille de route ?
pub async fn may_see_phones(
    db: &PgPool,
    scope: &CollectiveScope,
    event_id: Uuid,
) -> AppResult<bool> {
    if scope.is_admin() {
        return Ok(true);
    }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT count(*) FROM participations p
         LEFT JOIN group_members gm ON gm.group_id = p.group_id
         WHERE p.event_id = $1 AND (p.user_id = $2 OR gm.user_id = $2)",
    )
    .bind(event_id)
    .bind(scope.user_id())
    .fetch_optional(db)
    .await?;
    if row.map(|(n,)| n > 0).unwrap_or(false) {
        return Ok(true);
    }
    // Tenir un poste logistique suffit : on a besoin de joindre les autres.
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT count(*) FROM logistics_assignments a
         JOIN logistics_slots s ON s.id = a.slot_id
         WHERE s.event_id = $1 AND a.user_id = $2",
    )
    .bind(event_id)
    .bind(scope.user_id())
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(n,)| n > 0).unwrap_or(false))
}

pub async fn build(
    db: &PgPool,
    scope: &CollectiveScope,
    event_id: Uuid,
    with_phones: bool,
) -> AppResult<RunSheet> {
    let ev: Option<(
        Uuid,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
        Option<Uuid>,
        bool,
    )> = sqlx::query_as(
        "SELECT e.id, e.title, e.starts_at, e.doors_at, e.soundcheck_at, e.venue_id, e.set_times_public
         FROM events e WHERE e.id = $1 AND e.collective_id = $2",
    )
    .bind(event_id)
    .bind(scope.collective_id())
    .fetch_optional(db)
    .await?;
    let (id, title, starts_at, doors_at, soundcheck_at, venue_id, _set_public) =
        ev.ok_or_else(|| AppError::not_found("evenement introuvable"))?;

    let venue = match venue_id {
        Some(vid) => {
            let v: (String, Option<String>, Option<String>) =
                sqlx::query_as("SELECT name, address, city FROM venues WHERE id = $1")
                    .bind(vid)
                    .fetch_one(db)
                    .await?;
            let contacts: Vec<(String, Option<String>, Option<String>)> =
                sqlx::query_as("SELECT name, role, phone FROM venue_contacts WHERE venue_id = $1")
                    .bind(vid)
                    .fetch_all(db)
                    .await?;
            Some(Venue {
                name: v.0,
                address: v.1,
                city: v.2,
                contacts: contacts
                    .into_iter()
                    .map(|(name, role, phone)| Contact {
                        name,
                        role,
                        phone: phone.filter(|_| with_phones),
                    })
                    .collect(),
            })
        }
        None => None,
    };

    // Line-up : un groupe apporte ses membres, une participation individuelle
    // apporte la personne.
    let parts: Vec<(Uuid, Option<Uuid>, Option<String>, Option<Uuid>, Option<String>, Option<String>, Option<chrono::NaiveTime>, Option<chrono::NaiveTime>)> =
        sqlx::query_as(
            "SELECT p.id, p.group_id, g.name, p.user_id, u.display_name, p.stage_role, p.slot_start, p.slot_end
             FROM participations p
             LEFT JOIN groups g ON g.id = p.group_id
             LEFT JOIN users u ON u.id = p.user_id
             WHERE p.event_id = $1 AND p.status <> 'declined'
             ORDER BY p.position",
        )
        .bind(event_id)
        .fetch_all(db)
        .await?;

    let mut line_up = Vec::new();
    for (_pid, group_id, group_name, user_id, user_name, stage_role, s, e) in parts {
        let members = match (group_id, user_id) {
            (Some(gid), _) => {
                let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
                    "SELECT u.id, COALESCE(u.stage_name, u.display_name), u.phone
                     FROM group_members gm JOIN users u ON u.id = gm.user_id
                     WHERE gm.group_id = $1 ORDER BY u.display_name",
                )
                .bind(gid)
                .fetch_all(db)
                .await?;
                rows.into_iter()
                    .map(|(user_id, name, phone)| Person {
                        user_id,
                        name,
                        phone: phone.filter(|_| with_phones),
                    })
                    .collect()
            }
            (None, Some(uid)) => {
                let row: (String, Option<String>) = sqlx::query_as(
                    "SELECT COALESCE(stage_name, display_name), phone FROM users WHERE id = $1",
                )
                .bind(uid)
                .fetch_one(db)
                .await?;
                vec![Person {
                    user_id: uid,
                    name: row.0,
                    phone: row.1.filter(|_| with_phones),
                }]
            }
            _ => vec![],
        };
        line_up.push(LineUpLine {
            label: group_name.or(user_name).unwrap_or_else(|| "?".into()),
            stage_role,
            slot: match (s, e) {
                (Some(s), Some(e)) => Some(format!("{}–{}", s.format("%H:%M"), e.format("%H:%M"))),
                (Some(s), None) => Some(s.format("%H:%M").to_string()),
                _ => None,
            },
            members,
        });
    }

    let slots: Vec<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT id, label, quantity FROM logistics_slots WHERE event_id = $1 ORDER BY position",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    let mut logistics = Vec::new();
    for (slot_id, label, quantity) in slots {
        let rows: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
            "SELECT u.id, COALESCE(u.stage_name, u.display_name), u.phone
             FROM logistics_assignments a JOIN users u ON u.id = a.user_id
             WHERE a.slot_id = $1",
        )
        .bind(slot_id)
        .fetch_all(db)
        .await?;
        let assignees: Vec<Person> = rows
            .into_iter()
            .map(|(user_id, name, phone)| Person {
                user_id,
                name,
                phone: phone.filter(|_| with_phones),
            })
            .collect();
        let vacant = (quantity - assignees.len() as i32).max(0);
        logistics.push(LogisticsLine {
            label,
            quantity,
            assignees,
            vacant,
        });
    }

    let riders: Vec<(Uuid, String, Option<Uuid>, Option<i32>)> = sqlx::query_as(
        "SELECT etr.group_id, g.name, etr.tech_rider_id, r.version
         FROM event_tech_riders etr
         JOIN groups g ON g.id = etr.group_id
         LEFT JOIN tech_riders r ON r.id = etr.tech_rider_id
         WHERE etr.event_id = $1",
    )
    .bind(event_id)
    .fetch_all(db)
    .await?;

    Ok(RunSheet {
        event_id: id,
        title,
        starts_at,
        doors_at,
        soundcheck_at,
        venue,
        line_up,
        logistics,
        tech_riders: riders
            .into_iter()
            .map(|(group_id, group_name, tech_rider_id, version)| RiderLine {
                group_id,
                group_name,
                tech_rider_id,
                version,
            })
            .collect(),
    })
}

/// Version texte, envoyee par le bot la veille.
pub fn to_text(rs: &RunSheet) -> String {
    let mut out = String::new();
    let local = rs.starts_at.with_timezone(&Paris);
    out.push_str(&format!(
        "<b>{}</b>\n{}\n",
        rs.title,
        local.format("%A %d/%m a %H:%M")
    ));

    if let Some(v) = &rs.venue {
        out.push_str(&format!("\n📍 {}", v.name));
        if let Some(a) = &v.address {
            out.push_str(&format!("\n{a}"));
        }
        if let Some(c) = &v.city {
            out.push_str(&format!(" — {c}"));
        }
        for c in &v.contacts {
            let role = c.role.clone().unwrap_or_default();
            let phone = c.phone.clone().unwrap_or_else(|| "—".into());
            out.push_str(&format!("\n☎️ {} ({role}) {phone}", c.name));
        }
    }

    if let Some(sc) = rs.soundcheck_at {
        out.push_str(&format!(
            "\n\n🎚 Balance : {}",
            sc.with_timezone(&Paris).format("%H:%M")
        ));
    }
    if let Some(d) = rs.doors_at {
        out.push_str(&format!(
            "\n🚪 Ouverture : {}",
            d.with_timezone(&Paris).format("%H:%M")
        ));
    }

    if !rs.line_up.is_empty() {
        out.push_str("\n\n<b>Line-up</b>");
        for l in &rs.line_up {
            let slot = l
                .slot
                .clone()
                .map(|s| format!(" — {s}"))
                .unwrap_or_default();
            out.push_str(&format!("\n• {}{slot}", l.label));
        }
    }

    if !rs.logistics.is_empty() {
        out.push_str("\n\n<b>Postes</b>");
        for l in &rs.logistics {
            let who = if l.assignees.is_empty() {
                "VACANT".to_string()
            } else {
                l.assignees
                    .iter()
                    .map(|p| match &p.phone {
                        Some(ph) => format!("{} {}", p.name, ph),
                        None => p.name.clone(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            out.push_str(&format!("\n• {} : {who}", l.label));
        }
    }

    let missing: Vec<&str> = rs
        .tech_riders
        .iter()
        .filter(|r| r.tech_rider_id.is_none())
        .map(|r| r.group_name.as_str())
        .collect();
    if !missing.is_empty() {
        out.push_str(&format!(
            "\n\n⚠️ Fiche technique manquante : {}",
            missing.join(", ")
        ));
    }

    out
}
