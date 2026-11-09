//! Deletes raw ingest rows in batches. Issue counts and analytics daily
//! rollups stay, because those are what the dashboards read.

use owlee_platform::Database;

use crate::state::AppState;

const BATCH: i64 = 500;

pub struct PruneStats {
    pub error_events: u64,
    pub analytics_events: u64,
}

pub async fn prune_expired(state: &AppState) -> Result<PruneStats, sqlx::Error> {
    let now = chrono::Utc::now();
    let error_before = (now - chrono::Duration::days(state.config.ingest.error_retention_days))
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let analytics_before = now.timestamp()
        - state
            .config
            .ingest
            .analytics_retention_days
            .saturating_mul(86_400);
    prune_before(&state.db, &error_before, analytics_before).await
}

pub async fn prune_before(
    db: &Database,
    error_before: &str,
    analytics_before_unix: i64,
) -> Result<PruneStats, sqlx::Error> {
    let error_events = delete_batches(
        db,
        "DELETE FROM events WHERE id IN (SELECT id FROM events WHERE timestamp < $1 ORDER BY timestamp LIMIT 500)",
        error_before,
    )
    .await?;
    let analytics_events = delete_unix(
        db,
        "DELETE FROM insight_events WHERE id IN (SELECT id FROM insight_events WHERE timestamp < $1 ORDER BY timestamp LIMIT 500)",
        analytics_before_unix,
    )
    .await?;
    Ok(PruneStats {
        error_events,
        analytics_events,
    })
}

async fn delete_batches(db: &Database, sql: &str, before: &str) -> Result<u64, sqlx::Error> {
    let mut total = 0;
    loop {
        let deleted = sqlx::query(sql)
            .bind(before)
            .execute(db.pool())
            .await?
            .rows_affected();
        total += deleted;
        if deleted == 0 || deleted < BATCH as u64 {
            break;
        }
    }
    Ok(total)
}

async fn delete_unix(db: &Database, sql: &str, before: i64) -> Result<u64, sqlx::Error> {
    let mut total = 0;
    loop {
        let deleted = sqlx::query(sql)
            .bind(before)
            .execute(db.pool())
            .await?
            .rows_affected();
        total += deleted;
        if deleted == 0 || deleted < BATCH as u64 {
            break;
        }
    }
    Ok(total)
}
