use std::time::Duration;

use haruvo_platform::TimestampMillis;

use crate::engine;
use crate::error::AppError;
use crate::repo;
use crate::state::AppState;

const PRUNE_EVERY_MS: i64 = 3_600_000;
const RETENTION_MS: i64 = 14 * 86_400_000;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        let mut last_pruned = 0;
        loop {
            if let Err(error) = tick(&state, &mut last_pruned).await {
                tracing::error!(%error, "scheduler tick failed");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn tick(state: &AppState, last_pruned: &mut i64) -> Result<(), sqlx::Error> {
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
        *last_pruned = now;
    }
    Ok(())
}
