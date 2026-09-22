//! The `members x candidate dates` matrix (§5.2).
//!
//! This is the screen that removes the back and forth: for each date, how many
//! people are available, **which groups are complete** (and therefore playable
//! as they stand), and who is missing from the others.

use crate::error::AppResult;
use crate::scope::CollectiveScope;
use chrono::NaiveDate;
use serde::Serialize;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct Matrix {
    pub opportunity_id: Uuid,
    pub poll_open: bool,
    pub dates: Vec<MatrixDate>,
    pub members: Vec<MatrixMember>,
    pub groups: Vec<MatrixGroup>,
}

#[derive(Debug, Serialize)]
pub struct MatrixDate {
    pub id: Uuid,
    pub day: NaiveDate,
    pub start_time: Option<chrono::NaiveTime>,
    pub end_time: Option<chrono::NaiveTime>,
    pub notes: Option<String>,
    pub yes: i64,
    pub maybe: i64,
    pub no: i64,
    pub no_answer: i64,
    /// Members who pressed "🎸 je veux jouer" for this date.
    pub volunteers: Vec<Uuid>,
    /// Groups where **every** member is available: a possible line-up.
    pub complete_groups: Vec<Uuid>,
    /// Incomplete groups, naming who is missing.
    pub partial_groups: Vec<PartialGroup>,
}

#[derive(Debug, Serialize)]
pub struct PartialGroup {
    pub group_id: Uuid,
    pub missing: Vec<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct MatrixMember {
    pub user_id: Uuid,
    pub display_name: String,
    pub stage_name: Option<String>,
    pub group_ids: Vec<Uuid>,
    /// Answer per candidate date: `yes` | `maybe` | `no`, absent if silent.
    pub answers: HashMap<Uuid, Answer>,
}

#[derive(Debug, Serialize, Clone)]
pub struct Answer {
    pub status: String,
    pub wants_to_play: bool,
}

#[derive(Debug, Serialize)]
pub struct MatrixGroup {
    pub id: Uuid,
    pub name: String,
    pub member_ids: Vec<Uuid>,
}

pub async fn build(
    db: &PgPool,
    scope: &CollectiveScope,
    opportunity_id: Uuid,
) -> AppResult<Matrix> {
    // Isolation: the opportunity must belong to the scope's collective.
    let exists: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM opportunities WHERE id = $1 AND collective_id = $2")
            .bind(opportunity_id)
            .bind(scope.collective_id())
            .fetch_optional(db)
            .await?;
    if exists.is_none() {
        return Err(crate::error::AppError::not_found("opportunite introuvable"));
    }

    let poll: Option<(Uuid, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as("SELECT id, closed_at FROM availability_polls WHERE opportunity_id = $1")
            .bind(opportunity_id)
            .fetch_optional(db)
            .await?;

    let dates: Vec<(
        Uuid,
        NaiveDate,
        Option<chrono::NaiveTime>,
        Option<chrono::NaiveTime>,
        Option<String>,
    )> = sqlx::query_as(
        "SELECT id, day, start_time, end_time, notes FROM candidate_dates
             WHERE opportunity_id = $1 ORDER BY position, day",
    )
    .bind(opportunity_id)
    .fetch_all(db)
    .await?;

    let members: Vec<(Uuid, String, Option<String>)> = sqlx::query_as(
        "SELECT u.id, u.display_name, u.stage_name
         FROM memberships m JOIN users u ON u.id = m.user_id
         WHERE m.collective_id = $1
         ORDER BY u.display_name",
    )
    .bind(scope.collective_id())
    .fetch_all(db)
    .await?;

    let group_rows: Vec<(Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "SELECT g.id, g.name, gm.user_id
         FROM groups g LEFT JOIN group_members gm ON gm.group_id = g.id
         WHERE g.collective_id = $1
         ORDER BY g.name",
    )
    .bind(scope.collective_id())
    .fetch_all(db)
    .await?;

    let answers: Vec<(Uuid, Uuid, String, bool)> = sqlx::query_as(
        "SELECT a.candidate_date_id, a.user_id, a.status, a.wants_to_play
         FROM availabilities a
         JOIN candidate_dates d ON d.id = a.candidate_date_id
         WHERE d.opportunity_id = $1",
    )
    .bind(opportunity_id)
    .fetch_all(db)
    .await?;

    // --- assembly ---------------------------------------------------------
    let mut groups: Vec<MatrixGroup> = Vec::new();
    let mut by_group: HashMap<Uuid, usize> = HashMap::new();
    let mut user_groups: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (gid, name, uid) in group_rows {
        let idx = *by_group.entry(gid).or_insert_with(|| {
            groups.push(MatrixGroup {
                id: gid,
                name,
                member_ids: vec![],
            });
            groups.len() - 1
        });
        if let Some(uid) = uid {
            groups[idx].member_ids.push(uid);
            user_groups.entry(uid).or_default().push(gid);
        }
    }

    let mut per_date: HashMap<Uuid, HashMap<Uuid, Answer>> = HashMap::new();
    for (date_id, user_id, status, wants) in &answers {
        per_date.entry(*date_id).or_default().insert(
            *user_id,
            Answer {
                status: status.clone(),
                wants_to_play: *wants,
            },
        );
    }

    let member_ids: HashSet<Uuid> = members.iter().map(|(id, _, _)| *id).collect();

    let dates = dates
        .into_iter()
        .map(|(id, day, start_time, end_time, notes)| {
            let a = per_date.get(&id);
            let count = |s: &str| {
                a.map(|m| m.values().filter(|v| v.status == s).count() as i64)
                    .unwrap_or(0)
            };
            let (yes, maybe, no) = (count("yes"), count("maybe"), count("no"));
            let answered = a.map(|m| m.len() as i64).unwrap_or(0);

            let available = |uid: &Uuid| {
                a.and_then(|m| m.get(uid))
                    .map(|v| v.status == "yes")
                    .unwrap_or(false)
            };

            let mut complete_groups = Vec::new();
            let mut partial_groups = Vec::new();
            for g in &groups {
                // An empty group is neither complete nor incomplete: it has nothing to say.
                if g.member_ids.is_empty() {
                    continue;
                }
                let missing: Vec<Uuid> = g
                    .member_ids
                    .iter()
                    .copied()
                    .filter(|u| !available(u))
                    .collect();
                if missing.is_empty() {
                    complete_groups.push(g.id);
                } else {
                    partial_groups.push(PartialGroup {
                        group_id: g.id,
                        missing,
                    });
                }
            }

            let volunteers = a
                .map(|m| {
                    m.iter()
                        .filter(|(u, v)| v.wants_to_play && member_ids.contains(u))
                        .map(|(u, _)| *u)
                        .collect()
                })
                .unwrap_or_default();

            MatrixDate {
                id,
                day,
                start_time,
                end_time,
                notes,
                yes,
                maybe,
                no,
                no_answer: member_ids.len() as i64 - answered,
                volunteers,
                complete_groups,
                partial_groups,
            }
        })
        .collect();

    let members = members
        .into_iter()
        .map(|(user_id, display_name, stage_name)| {
            let answers = per_date
                .iter()
                .filter_map(|(date_id, m)| m.get(&user_id).map(|a| (*date_id, a.clone())))
                .collect();
            MatrixMember {
                user_id,
                display_name,
                stage_name,
                group_ids: user_groups.get(&user_id).cloned().unwrap_or_default(),
                answers,
            }
        })
        .collect();

    Ok(Matrix {
        opportunity_id,
        poll_open: matches!(poll, Some((_, None))),
        dates,
        members,
        groups,
    })
}
