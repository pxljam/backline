//! Secret-token iCal feeds (§7). The application is the **master**: no OAuth,
//! no two-way sync — a subscription, and that is all.

use crate::error::AppResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

pub struct IcalEvent {
    pub id: Uuid,
    pub title: String,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub status: String,
    pub venue: Option<String>,
    pub city: Option<String>,
    pub notes: Option<String>,
    /// `true` for a candidate date still under arbitration: it comes out as
    /// `TENTATIVE`, and the client calendar shows it dotted.
    pub tentative: bool,
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

fn stamp(dt: DateTime<Utc>) -> String {
    dt.format("%Y%m%dT%H%M%SZ").to_string()
}

/// Folds lines at 75 bytes as RFC 5545 requires — Google Calendar silently
/// rejects feeds that do not.
fn fold(line: &str) -> String {
    let bytes = line.as_bytes();
    if bytes.len() <= 75 {
        return line.to_string();
    }
    let mut out = String::new();
    let mut count = 0usize;
    for ch in line.chars() {
        let w = ch.len_utf8();
        if count + w > 75 {
            out.push_str("\r\n ");
            count = 1;
        }
        out.push(ch);
        count += w;
    }
    out
}

pub fn render(calendar_name: &str, events: &[IcalEvent]) -> String {
    let mut lines = vec![
        "BEGIN:VCALENDAR".to_string(),
        "VERSION:2.0".to_string(),
        "PRODID:-//Backline//BCKLN//FR".to_string(),
        "CALSCALE:GREGORIAN".to_string(),
        "METHOD:PUBLISH".to_string(),
        format!("X-WR-CALNAME:{}", escape(calendar_name)),
        "X-WR-TIMEZONE:Europe/Paris".to_string(),
    ];

    for e in events {
        let end = e
            .ends_at
            .unwrap_or(e.starts_at + chrono::Duration::hours(3));
        let location = match (&e.venue, &e.city) {
            (Some(v), Some(c)) => format!("{v}, {c}"),
            (Some(v), None) => v.clone(),
            (None, Some(c)) => c.clone(),
            _ => String::new(),
        };
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}@backline", e.id));
        lines.push(format!("DTSTAMP:{}", stamp(Utc::now())));
        lines.push(format!("DTSTART:{}", stamp(e.starts_at)));
        lines.push(format!("DTEND:{}", stamp(end)));
        lines.push(format!("SUMMARY:{}", escape(&e.title)));
        if !location.is_empty() {
            lines.push(format!("LOCATION:{}", escape(&location)));
        }
        if let Some(n) = &e.notes {
            if !n.is_empty() {
                lines.push(format!("DESCRIPTION:{}", escape(n)));
            }
        }
        lines.push(format!(
            "STATUS:{}",
            if e.tentative {
                "TENTATIVE"
            } else if e.status == "cancelled" {
                "CANCELLED"
            } else {
                "CONFIRMED"
            }
        ));
        lines.push("END:VEVENT".into());
    }

    lines.push("END:VCALENDAR".into());
    lines
        .iter()
        .map(|l| fold(l))
        .collect::<Vec<_>>()
        .join("\r\n")
        + "\r\n"
}

/// Creates the feed's token if it does not exist yet. Idempotent.
pub async fn ensure_token(db: &PgPool, scope: &str, scope_id: Uuid) -> AppResult<String> {
    if let Some((token,)) = sqlx::query_as::<_, (String,)>(
        "SELECT token FROM ical_tokens WHERE scope = $1 AND scope_id = $2",
    )
    .bind(scope)
    .bind(scope_id)
    .fetch_optional(db)
    .await?
    {
        return Ok(token);
    }
    let token = crate::auth::session::random_token();
    let (token,): (String,) = sqlx::query_as(
        "INSERT INTO ical_tokens (scope, scope_id, token) VALUES ($1, $2, $3)
         ON CONFLICT (scope, scope_id) DO UPDATE SET scope = EXCLUDED.scope
         RETURNING token",
    )
    .bind(scope)
    .bind(scope_id)
    .bind(&token)
    .fetch_one(db)
    .await?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_a_valid_calendar_and_folds_long_lines() {
        let ev = IcalEvent {
            id: Uuid::nil(),
            title: "Bonsoir Techno x Ramas — une soiree au titre volontairement tres long pour depasser la limite".into(),
            starts_at: Utc::now(),
            ends_at: None,
            status: "confirmed".into(),
            venue: Some("Le Sonic".into()),
            city: Some("Lyon".into()),
            notes: None,
            tentative: false,
        };
        let out = render("Bonsoir Techno", &[ev]);
        assert!(out.starts_with("BEGIN:VCALENDAR\r\n"));
        assert!(out.ends_with("END:VCALENDAR\r\n"));
        assert!(out.contains("STATUS:CONFIRMED"));
        assert!(
            out.contains("\r\n "),
            "les lignes longues doivent etre pliees"
        );
        for line in out.split("\r\n") {
            assert!(line.len() <= 75, "ligne trop longue: {line}");
        }
    }

    #[test]
    fn a_candidate_date_comes_out_as_tentative() {
        let ev = IcalEvent {
            id: Uuid::nil(),
            title: "Date candidate".into(),
            starts_at: Utc::now(),
            ends_at: None,
            status: "draft".into(),
            venue: None,
            city: None,
            notes: None,
            tentative: true,
        };
        assert!(render("x", &[ev]).contains("STATUS:TENTATIVE"));
    }
}
