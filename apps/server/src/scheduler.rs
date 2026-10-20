use std::time::Duration;

use status_platform::TimestampMillis;

use crate::engine;
use crate::error::AppError;
use crate::repo;
use crate::state::AppState;

pub fn spawn(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = tick(&state).await {
                tracing::error!(%error, "scheduler tick failed");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn tick(state: &AppState) -> Result<(), sqlx::Error> {
    let now = TimestampMillis::now().as_millis();
    if !repo::try_scheduler_lock(&state.db, &state.scheduler_id, now).await? {
        return Ok(());
    }
    let due = repo::due_monitors(&state.db, now, 20).await?;
    let mut joins = tokio::task::JoinSet::new();
    for monitor in due {
        let next = now + monitor.interval_seconds.saturating_mul(1000);
        if !repo::claim_monitor(&state.db, monitor.id, now, next).await? {
            continue;
        }
        let state = state.clone();
        let id = monitor.id;
        joins.spawn(async move {
            match engine::execute(&state, id).await {
                Ok(()) | Err(AppError::Busy) | Err(AppError::NotFound) => {}
                Err(error) => tracing::error!(%error, %id, "monitor check failed"),
            }
        });
    }
    while joins.join_next().await.is_some() {}
    if now % 3_600_000 < 2_000 {
        repo::prune(&state.db, now - 14 * 86_400_000).await?;
    }
    Ok(())
}
