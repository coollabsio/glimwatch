//! Per-project and per-site daily ingest quotas.
//!
//! The number is the instance setting (Settings, or `OWLEE_*_DAILY_QUOTA`
//! until that row is saved). The counter is per subject, so one project
//! cannot spend another's allowance.

use chrono::{Duration, Utc};
use owlee_platform::Database;
use sqlx::Row;

use crate::error::AppError;

pub const ERROR: &str = "error";
pub const ANALYTICS: &str = "analytics";

pub struct Charge {
    pub allowed: bool,
    pub retry_after_secs: u64,
    #[allow(dead_code)]
    pub used: i64,
    #[allow(dead_code)]
    pub quota: i64,
}

pub async fn error_quota(db: &Database) -> Result<i64, AppError> {
    let (error, _) = limits(db).await?;
    Ok(error)
}

pub async fn analytics_quota(db: &Database) -> Result<i64, AppError> {
    let (_, analytics) = limits(db).await?;
    Ok(analytics)
}

pub async fn limits(db: &Database) -> Result<(i64, i64), AppError> {
    let row = sqlx::query(
        "SELECT error_daily_quota, analytics_daily_quota FROM instance_settings WHERE id = 1",
    )
    .fetch_one(db.pool())
    .await?;
    Ok((
        row.try_get::<i64, _>("error_daily_quota")?,
        row.try_get::<i64, _>("analytics_daily_quota")?,
    ))
}

pub async fn used(db: &Database, kind: &str, subject_id: &str) -> Result<i64, AppError> {
    let row = sqlx::query(
        "SELECT count FROM ingest_usage WHERE kind = $1 AND subject_id = $2 AND day = $3",
    )
    .bind(kind)
    .bind(subject_id)
    .bind(today())
    .fetch_optional(db.pool())
    .await?;
    Ok(row
        .map(|row| row.try_get::<i64, _>("count"))
        .transpose()?
        .unwrap_or(0))
}

/// Reserves one event. `allowed` is false when today's count is already at the quota.
pub async fn charge(
    db: &Database,
    kind: &str,
    subject_id: &str,
    quota: i64,
) -> Result<Charge, AppError> {
    let day = today();
    let mut tx = db.pool().begin().await?;
    let row = sqlx::query(
        "SELECT count FROM ingest_usage WHERE kind = $1 AND subject_id = $2 AND day = $3",
    )
    .bind(kind)
    .bind(subject_id)
    .bind(&day)
    .fetch_optional(&mut *tx)
    .await?;
    let used = row
        .map(|row| row.try_get::<i64, _>("count"))
        .transpose()?
        .unwrap_or(0);
    if used >= quota {
        tx.commit().await?;
        return Ok(Charge {
            allowed: false,
            retry_after_secs: secs_until_utc_midnight(),
            used,
            quota,
        });
    }
    sqlx::query(
        "INSERT INTO ingest_usage (kind, subject_id, day, count) VALUES ($1, $2, $3, 1) \
         ON CONFLICT (kind, subject_id, day) DO UPDATE SET count = ingest_usage.count + 1",
    )
    .bind(kind)
    .bind(subject_id)
    .bind(&day)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Charge {
        allowed: true,
        retry_after_secs: 0,
        used: used + 1,
        quota,
    })
}

pub fn today() -> String {
    Utc::now().format("%Y-%m-%d").to_string()
}

pub fn secs_until_utc_midnight() -> u64 {
    let now = Utc::now();
    let next = (now.date_naive() + Duration::days(1))
        .and_hms_opt(0, 0, 0)
        .unwrap_or(now.naive_utc());
    let seconds = (next - now.naive_utc()).num_seconds();
    u64::try_from(seconds.max(1)).unwrap_or(1)
}
