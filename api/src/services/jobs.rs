//! Queue and scheduling (§15).
//!
//! One PostgreSQL table, `FOR UPDATE SKIP LOCKED`. No Redis, no extra service —
//! and a reminder scheduled at D-30 survives a restart. **The same mechanism
//! serves the render CLI** (`render_jobs`): one queue to understand.

use crate::error::AppResult;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Job {
    pub id: Uuid,
    pub kind: String,
    pub payload: Value,
    pub attempts: i32,
}

const MAX_ATTEMPTS: i32 = 5;

/// Schedules a job. `dedupe_key` makes the call idempotent: rescheduling an
/// event's D-7 reminder does not create a second send.
pub async fn enqueue(
    db: &PgPool,
    kind: &str,
    run_at: DateTime<Utc>,
    payload: Value,
    dedupe_key: Option<String>,
) -> AppResult<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO jobs (kind, run_at, payload, dedupe_key)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (dedupe_key) DO NOTHING
         RETURNING id",
    )
    .bind(kind)
    .bind(run_at)
    .bind(&payload)
    .bind(&dedupe_key)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(id,)| id))
}

/// Claims up to `limit` due jobs. Several API instances can run without
/// treading on each other.
pub async fn claim(db: &PgPool, limit: i64) -> AppResult<Vec<Job>> {
    let jobs: Vec<Job> = sqlx::query_as(
        "UPDATE jobs SET status = 'running', locked_at = now(), attempts = attempts + 1
         WHERE id IN (
             SELECT id FROM jobs
             WHERE status = 'pending' AND run_at <= now()
             ORDER BY run_at
             FOR UPDATE SKIP LOCKED
             LIMIT $1
         )
         RETURNING id, kind, payload, attempts",
    )
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(jobs)
}

pub async fn mark_done(db: &PgPool, id: Uuid) -> AppResult<()> {
    sqlx::query("UPDATE jobs SET status = 'done', finished_at = now() WHERE id = $1")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Reschedules with exponential backoff, then gives up and says so.
pub async fn mark_failed(db: &PgPool, job: &Job, error: &str) -> AppResult<()> {
    if job.attempts >= MAX_ATTEMPTS {
        sqlx::query(
            "UPDATE jobs SET status = 'failed', last_error = $2, finished_at = now() WHERE id = $1",
        )
        .bind(job.id)
        .bind(error)
        .execute(db)
        .await?;
        tracing::error!(job = %job.id, kind = %job.kind, error, "job abandoned");
    } else {
        let backoff = chrono::Duration::seconds(30 * (1 << job.attempts.min(6)));
        sqlx::query(
            "UPDATE jobs SET status = 'pending', last_error = $2, run_at = now() + $3 WHERE id = $1",
        )
        .bind(job.id)
        .bind(error)
        .bind(backoff)
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Cancels an event's pending jobs (cancellation, moved date).
pub async fn cancel_by_prefix(db: &PgPool, dedupe_prefix: &str) -> AppResult<u64> {
    let r = sqlx::query(
        "UPDATE jobs SET status = 'done', finished_at = now()
         WHERE status = 'pending' AND dedupe_key LIKE $1 || '%'",
    )
    .bind(dedupe_prefix)
    .execute(db)
    .await?;
    Ok(r.rows_affected())
}
