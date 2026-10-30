use std::ops::DerefMut;

use owlee_domain::{
    CheckStatus, IncidentImpact, IncidentStatus, NotificationEvent, NotificationPayload,
    PayloadCheck, PayloadIncident, PayloadMonitor, Probe, Rules, apply_outcome, judge,
};
use owlee_platform::{Id, TimestampMillis};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::checker::{self, Job};
use crate::error::AppError;
use crate::notify;
use crate::repo::{self, Incident, IncidentUpdate, Monitor, NewCheck};
use crate::state::AppState;

pub const LOCAL_REGION: &str = "local";

/// A judged check result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Outcome {
    pub status: CheckStatus,
    pub status_code: Option<i64>,
    pub latency_ms: i64,
    pub error: Option<String>,
}

impl Outcome {
    fn maintenance() -> Self {
        Self {
            status: CheckStatus::Maintenance,
            status_code: None,
            latency_ms: 0,
            error: Some("Skipped during a maintenance window.".to_owned()),
        }
    }
}

pub fn judge_probe(rules: &Rules, probe: &Probe) -> Outcome {
    let judgement = judge(rules, probe);
    Outcome {
        status: judgement.status,
        status_code: probe.status_code,
        latency_ms: probe.latency_ms,
        error: judgement.error,
    }
}

/// Runs a check from this server and records it.
pub async fn execute(state: &AppState, id: Id) -> Result<(), AppError> {
    let monitor = active_monitor(state, id).await?;
    let _guard = state
        .inflight
        .try_acquire(&id.to_string())
        .ok_or(AppError::Busy)?;
    let now = TimestampMillis::now();
    let outcome = if repo::in_maintenance(&state.db, id, now.as_millis()).await? {
        Outcome::maintenance()
    } else {
        let probe = checker::probe(&state.http, &Job::from(&monitor)).await;
        if probe.transport_error.is_some() && !state.connectivity.online().await {
            return Err(AppError::Offline);
        }
        judge_probe(
            &Rules {
                job_type: monitor.job_type,
                expected_status: monitor.expected_status,
                degraded_after_ms: monitor.degraded_after_ms,
                assertions: monitor.assertions.clone(),
            },
            &probe,
        )
    };
    record(state, &monitor, outcome, LOCAL_REGION, now).await
}

/// Records an outcome that an agent judged inside its network.
pub async fn submit(
    state: &AppState,
    id: Id,
    outcome: Outcome,
    region: &str,
) -> Result<(), AppError> {
    let monitor = active_monitor(state, id).await?;
    let _guard = state
        .inflight
        .try_acquire(&id.to_string())
        .ok_or(AppError::Busy)?;
    let now = TimestampMillis::now();
    let outcome = if repo::in_maintenance(&state.db, id, now.as_millis()).await? {
        Outcome::maintenance()
    } else {
        outcome
    };
    record(state, &monitor, outcome, region, now).await
}

async fn active_monitor(state: &AppState, id: Id) -> Result<Monitor, AppError> {
    let monitor = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !monitor.active {
        return Err(AppError::Conflict(
            "Resume the monitor before running a check.".into(),
        ));
    }
    Ok(monitor)
}

async fn record(
    state: &AppState,
    monitor: &Monitor,
    outcome: Outcome,
    region: &str,
    now: TimestampMillis,
) -> Result<(), AppError> {
    let id = monitor.id;
    let Outcome {
        status,
        status_code,
        latency_ms,
        error,
    } = outcome;
    let transition = apply_outcome(
        monitor.state,
        monitor.consecutive_failures,
        monitor.failure_threshold,
        status,
    );
    let check = NewCheck {
        id: Id::new(),
        monitor_id: id,
        status,
        status_code,
        latency_ms,
        error: error.clone(),
        region: region.to_owned(),
        checked_at: now,
    };
    let next = now.as_millis() + monitor.interval_seconds.saturating_mul(1000);
    let mut tx = state.db.begin().await?;
    repo::insert_check(&mut tx, &check).await?;
    repo::bump_daily(&mut tx, id, now.unix_day(), status, latency_ms).await?;
    repo::save_outcome(
        &mut tx,
        id,
        transition.state,
        transition.consecutive_failures,
        now.as_millis(),
        next,
    )
    .await?;

    let mut notify_event = None;
    if transition.open_incident {
        let open = sqlx::query(
            "SELECT id FROM incidents WHERE monitor_id = $1 AND auto = 1 AND status != 'resolved' LIMIT 1",
        )
        .bind(id.to_string())
        .fetch_optional(tx.deref_mut())
        .await?;
        if open.is_none() {
            let incident_id = Id::new();
            let detail = error.unwrap_or_else(|| "The check failed.".to_owned());
            let message = format!(
                "Owlee recorded {} consecutive failed checks for {}. {detail}",
                transition.consecutive_failures, monitor.url
            );
            let incident = Incident {
                id: incident_id,
                page_id: None,
                monitor_id: Some(id),
                title: format!("{} is down", monitor.name),
                status: IncidentStatus::Investigating,
                impact: IncidentImpact::Major,
                auto: true,
                started_at: now,
                resolved_at: None,
                created_at: now,
                updated_at: now,
            };
            let update = IncidentUpdate {
                id: Id::new(),
                incident_id,
                status: IncidentStatus::Investigating,
                message: message.clone(),
                created_at: now,
            };
            repo::insert_incident(&mut tx, &incident, &update).await?;
            notify_event = Some((NotificationEvent::MonitorDown, incident_id, message));
        }
    } else if transition.resolve_incident {
        let open = sqlx::query(
            "SELECT id FROM incidents WHERE monitor_id = $1 AND auto = 1 AND status != 'resolved' \
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(id.to_string())
        .fetch_optional(tx.deref_mut())
        .await?;
        if let Some(row) = open {
            let incident_id = row
                .try_get::<String, _>("id")
                .ok()
                .and_then(|value| value.parse().ok());
            if let Some(incident_id) = incident_id {
                let message = format!(
                    "Checks for {} are succeeding again. This incident was resolved automatically.",
                    monitor.name
                );
                repo::apply_incident_status(
                    &mut tx,
                    incident_id,
                    IncidentStatus::Resolved,
                    Some(now.as_millis()),
                    now.as_millis(),
                )
                .await?;
                repo::insert_update(
                    &mut tx,
                    &IncidentUpdate {
                        id: Id::new(),
                        incident_id,
                        status: IncidentStatus::Resolved,
                        message: message.clone(),
                        created_at: now,
                    },
                )
                .await?;
                notify_event = Some((NotificationEvent::MonitorRecovered, incident_id, message));
            }
        }
    }
    tx.commit().await?;

    if transition.state != monitor.state {
        tracing::info!(
            monitor = %monitor.name,
            from = monitor.state.as_str(),
            to = transition.state.as_str(),
            "monitor state changed"
        );
    }
    if let Some((event, incident_id, message)) = notify_event {
        let recovered = event == NotificationEvent::MonitorRecovered;
        let payload = NotificationPayload {
            event: event.as_str().to_owned(),
            sent_at: now.to_string(),
            monitor: Some(PayloadMonitor {
                id: id.to_string(),
                name: monitor.name.clone(),
                url: monitor.url.clone(),
                state: transition.state,
            }),
            check: Some(PayloadCheck {
                status,
                status_code,
                latency_ms,
                error: check.error.clone(),
            }),
            incident: Some(PayloadIncident {
                id: incident_id.to_string(),
                title: format!("{} is down", monitor.name),
                status: if recovered {
                    IncidentStatus::Resolved
                } else {
                    IncidentStatus::Investigating
                },
                impact: IncidentImpact::Major,
            }),
            message: Some(message),
        };
        notify::fanout(state, &payload, Some(id), Some(incident_id)).await;
    }
    Ok(())
}
