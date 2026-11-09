use std::time::Duration;

use owlee_platform::TimestampMillis;

use crate::engine;
use crate::error::AppError;
use crate::repo;
use crate::state::AppState;

const PRUNE_EVERY_MS: i64 = 3_600_000;
const RETENTION_MS: i64 = 14 * 86_400_000;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut last_pruned = 0;
        let mut last_recheck = TimestampMillis::now().as_millis();
        loop {
            if let Err(error) = tick(&state, &mut last_pruned, &mut last_recheck).await {
                tracing::error!(%error, "scheduler tick failed");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn tick(
    state: &AppState,
    last_pruned: &mut i64,
    last_recheck: &mut i64,
) -> Result<(), sqlx::Error> {
    let now = TimestampMillis::now().as_millis();
    if !repo::try_scheduler_lock(&state.db, &state.scheduler_id, now).await? {
        return Ok(());
    }
    let due = repo::due_monitors(&state.db, now, 20).await?;
    for monitor in due {
        let next = now + monitor.interval_seconds.saturating_mul(1000);
        if !repo::claim_monitor(&state.db, monitor.id, now, next).await? {
            continue;
        }
        let state = state.clone();
        let id = monitor.id;
        // Detached so a slow probe does not delay the next tick.
        tokio::spawn(async move {
            match engine::execute(&state, id).await {
                Ok(()) | Err(AppError::Busy | AppError::NotFound | AppError::Offline) => {}
                Err(error) => tracing::error!(%error, %id, "monitor check failed"),
            }
        });
    }
    if now - *last_pruned >= PRUNE_EVERY_MS {
        repo::prune(&state.db, now - RETENTION_MS).await?;
        if let Err(error) = crate::retention::prune_expired(state).await {
            tracing::error!(%error, "ingest retention failed");
        }
        *last_pruned = now;
    }
    let recheck_ms = state.config.ingest.domain_recheck_secs.saturating_mul(1000);
    if now - *last_recheck >= recheck_ms {
        match crate::verify::recheck_verified(state).await {
            Ok(cleared) if cleared > 0 => tracing::info!(cleared, "domains lost TXT verification"),
            Ok(_) => {}
            Err(error) => tracing::error!(%error, "domain re-check failed"),
        }
        *last_recheck = now;
    }
    Ok(())
}
