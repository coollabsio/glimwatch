use std::ops::DerefMut;

use coolstatus_domain::{
    CheckStatus, IncidentImpact, IncidentStatus, NotificationEvent, NotificationPayload,
    PayloadCheck, PayloadIncident, PayloadMonitor, Rules, apply_outcome, judge,
};
use coolstatus_platform::{Id, TimestampMillis};
use sqlx::Row;

use crate::checker;
use crate::error::AppError;
use crate::notify;
use crate::repo::{self, Incident, IncidentUpdate, NewCheck};
use crate::state::AppState;

pub async fn execute(state: &AppState, id: Id) -> Result<(), AppError> {
    let monitor = repo::get_monitor(&state.db, id)
        .await?
        .ok_or(AppError::NotFound)?;
    if !monitor.active {
        return Err(AppError::Conflict(
            "Resume the monitor before running a check.".into(),
        ));
    }
    let _guard = state
        .inflight
        .try_acquire(&id.to_string())
        .ok_or(AppError::Busy)?;
    let now = TimestampMillis::now();
    let maintenance = repo::in_maintenance(&state.db, id, now.as_millis()).await?;
    let (status, status_code, latency_ms, error) = if maintenance {
        (
            CheckStatus::Maintenance,
            None,
            0,
            Some("Skipped during a maintenance window.".to_owned()),
        )
    } else {
        let probe = checker::probe(&state.http, &monitor).await;
        let judgement = judge(
            &Rules {
                job_type: monitor.job_type,
                expected_status: monitor.expected_status,
                degraded_after_ms: monitor.degraded_after_ms,
                assertions: monitor.assertions.clone(),
            },
            &probe,
        );
        (
            judgement.status,
            probe.status_code,
            probe.latency_ms,
            judgement.error,
        )
    };
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
                "CoolStatus recorded {} consecutive failed checks for {}. {detail}",
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
