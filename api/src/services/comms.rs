//! Comms plan (§11). Timelines are **data** carried by the event type: they are
//! edited in the collective's settings, never in the code.

use crate::error::AppResult;
use chrono::{DateTime, Duration, NaiveTime, TimeZone, Utc};
use chrono_tz::Europe::Paris;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Milestone {
    pub key: String,
    pub label: String,
    /// Offset in days from the anchor. Negative = before.
    #[serde(default)]
    pub offset_days: i64,
    /// Fine offset, used by streams ("D0 minus 15 min").
    #[serde(default)]
    pub offset_minutes: i64,
    /// Fixed local time; otherwise the anchor's time is kept.
    #[serde(default)]
    pub at: Option<String>,
    /// Format catalogue keys (§9.2) expected for this milestone.
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub caption: String,
    /// `start` (default) or `end` — a residency communicates from its end.
    #[serde(default = "anchor_start")]
    pub anchor: String,
}

fn anchor_start() -> String {
    "start".into()
}

impl Milestone {
    /// Trigger instant, computed in Paris time then brought back to UTC.
    pub fn scheduled_at(
        &self,
        starts_at: DateTime<Utc>,
        ends_at: Option<DateTime<Utc>>,
    ) -> DateTime<Utc> {
        let anchor = if self.anchor == "end" {
            ends_at.unwrap_or(starts_at)
        } else {
            starts_at
        };
        let local = anchor.with_timezone(&Paris);
        let shifted = local + Duration::days(self.offset_days);

        let with_time = match self.at.as_deref().and_then(parse_hm) {
            Some(t) => {
                let naive = shifted.date_naive().and_time(t);
                // Daylight saving: a local time may not exist, or may be
                // ambiguous. Take the first valid occurrence.
                Paris
                    .from_local_datetime(&naive)
                    .earliest()
                    .unwrap_or_else(|| Paris.from_utc_datetime(&naive))
            }
            None => shifted,
        };

        (with_time + Duration::minutes(self.offset_minutes)).with_timezone(&Utc)
    }
}

fn parse_hm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s, "%H:%M").ok()
}

/// Default timelines (§11.1). They seed a collective at creation time; each
/// collective can then rewrite them entirely.
pub fn default_timeline(type_key: &str) -> Vec<Milestone> {
    let m = |key: &str, label: &str, offset_days: i64, at: &str, formats: &[&str]| Milestone {
        key: key.into(),
        label: label.into(),
        offset_days,
        offset_minutes: 0,
        at: Some(at.into()),
        formats: formats.iter().map(|s| s.to_string()).collect(),
        caption: String::new(),
        anchor: anchor_start(),
    };

    match type_key {
        // Concerts and DJ nights share the same long timeline.
        "concert" | "dj_night" => vec![
            m(
                "j-30",
                "Annonce",
                -30,
                "18:00",
                &["ig_portrait", "ig_story"],
            ),
            m(
                "j-21",
                "Focus artiste / line-up",
                -21,
                "18:00",
                &["ig_square"],
            ),
            m(
                "j-14",
                "Extrait audio ou video",
                -14,
                "18:00",
                &["reel_9_16"],
            ),
            m(
                "j-7",
                "Rappel + infos pratiques",
                -7,
                "18:00",
                &["ig_portrait", "ig_story"],
            ),
            m(
                "j-3",
                "Teaser court",
                -3,
                "18:00",
                &["reel_9_16", "tiktok_9_16"],
            ),
            m("j-1", "Demain", -1, "18:00", &["ig_story"]),
            m("j0-matin", "Ce soir — story", 0, "10:00", &["ig_story"]),
            m("j0", "Ce soir — post", 0, "17:00", &["ig_portrait"]),
            m(
                "j+2",
                "Remerciements / photos",
                2,
                "18:00",
                &["ig_portrait", "ig_story"],
            ),
        ],
        // Residency: lighter timeline, the showing anchored on the end.
        "residency" => vec![
            m(
                "j-14",
                "Annonce de la residence",
                -14,
                "18:00",
                &["ig_portrait", "ig_story"],
            ),
            m("j-7", "On entre en residence", -7, "18:00", &["ig_story"]),
            m("coulisses-1", "Coulisses 1", 1, "18:00", &["ig_story"]),
            m("coulisses-2", "Coulisses 2", 3, "18:00", &["ig_story"]),
            Milestone {
                key: "j+3".into(),
                label: "Restitution".into(),
                offset_days: 3,
                offset_minutes: 0,
                at: Some("18:00".into()),
                formats: vec!["ig_portrait".into()],
                caption: String::new(),
                anchor: "end".into(),
            },
        ],
        // Stream: short cycle, responsiveness comes first.
        "stream" => vec![
            m(
                "j-7",
                "Annonce du live",
                -7,
                "18:00",
                &["ig_portrait", "ig_story"],
            ),
            m("j-3", "Teaser / invite", -3, "18:00", &["reel_9_16"]),
            m("j-1", "Rappel", -1, "18:00", &["ig_story"]),
            Milestone {
                key: "j0-2h".into(),
                label: "Ce soir en live".into(),
                offset_days: 0,
                offset_minutes: -120,
                at: None,
                formats: vec!["ig_story".into()],
                caption: String::new(),
                anchor: anchor_start(),
            },
            Milestone {
                key: "j0-15m".into(),
                label: "On est en ligne".into(),
                offset_days: 0,
                offset_minutes: -15,
                at: None,
                formats: vec!["ig_story".into()],
                caption: String::new(),
                anchor: anchor_start(),
            },
            Milestone {
                key: "pendant".into(),
                label: "Repartage du lien".into(),
                offset_days: 0,
                offset_minutes: 45,
                at: None,
                formats: vec!["ig_story".into()],
                caption: String::new(),
                anchor: anchor_start(),
            },
            m(
                "j+1",
                "Replay disponible",
                1,
                "12:00",
                &["ig_portrait", "ig_story"],
            ),
            m("j+3", "Extrait court du live", 3, "18:00", &["reel_9_16"]),
        ],
        _ => vec![],
    }
}

/// Logistics slots created empty on confirmation (§5.3).
pub fn default_logistics(type_key: &str) -> Vec<(&'static str, i32)> {
    match type_key {
        "concert" => vec![
            ("son", 1),
            ("lumiere", 1),
            ("transport backline", 2),
            ("photo", 1),
        ],
        "dj_night" => vec![("regie son", 1), ("lumiere", 1), ("bar", 2), ("photo", 1)],
        "residency" => vec![("transport materiel", 1), ("captation", 1)],
        "stream" => vec![
            ("camera", 2),
            ("lumiere", 1),
            ("regie / encodage", 1),
            ("gestion du chat", 1),
            ("connexion", 1),
        ],
        _ => vec![],
    }
}

/// Instantiates a confirmed event's comms plan. Every task is born in the
/// `draft` status: a human always approves (§11.1).
pub async fn instantiate_plan(db: &PgPool, event_id: Uuid) -> AppResult<Uuid> {
    let (starts_at, ends_at, milestones): (DateTime<Utc>, Option<DateTime<Utc>>, Value) =
        sqlx::query_as(
            "SELECT e.starts_at, e.ends_at, t.comms_milestones
             FROM events e JOIN event_types t ON t.id = e.event_type_id
             WHERE e.id = $1",
        )
        .bind(event_id)
        .fetch_one(db)
        .await?;

    let (plan_id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO comms_plans (event_id) VALUES ($1)
         ON CONFLICT (event_id) DO UPDATE SET event_id = EXCLUDED.event_id
         RETURNING id",
    )
    .bind(event_id)
    .fetch_one(db)
    .await?;

    let milestones: Vec<Milestone> = serde_json::from_value(milestones).unwrap_or_default();
    for ms in &milestones {
        let at = ms.scheduled_at(starts_at, ends_at);
        sqlx::query(
            "INSERT INTO publication_tasks
                (comms_plan_id, event_id, milestone_key, label, scheduled_at, caption, formats)
             VALUES ($1, $2, $3, $4, $5, $6, $7)
             ON CONFLICT DO NOTHING",
        )
        .bind(plan_id)
        .bind(event_id)
        .bind(&ms.key)
        .bind(&ms.label)
        .bind(at)
        .bind(&ms.caption)
        .bind(serde_json::to_value(&ms.formats).unwrap_or(Value::Null))
        .execute(db)
        .await?;
    }

    Ok(plan_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utc(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn d_minus_30_falls_30_days_before_at_the_stated_hour() {
        let ms = &default_timeline("concert")[0];
        // 2026-06-20 20:00 Paris (= 18:00 UTC in summer)
        let at = ms.scheduled_at(utc("2026-06-20T18:00:00Z"), None);
        let local = at.with_timezone(&Paris);
        assert_eq!(local.date_naive().to_string(), "2026-05-21");
        assert_eq!(local.format("%H:%M").to_string(), "18:00");
    }

    #[test]
    fn the_fifteen_minute_milestone_really_precedes_the_live() {
        let ms = default_timeline("stream")
            .into_iter()
            .find(|m| m.key == "j0-15m")
            .unwrap();
        let start = utc("2026-03-05T20:00:00Z");
        assert_eq!(ms.scheduled_at(start, None), start - Duration::minutes(15));
    }

    #[test]
    fn a_residency_showing_anchors_on_the_end() {
        let ms = default_timeline("residency")
            .into_iter()
            .find(|m| m.key == "j+3")
            .unwrap();
        let at = ms.scheduled_at(
            utc("2026-04-01T09:00:00Z"),
            Some(utc("2026-04-05T18:00:00Z")),
        );
        assert_eq!(
            at.with_timezone(&Paris).date_naive().to_string(),
            "2026-04-08"
        );
    }
}
