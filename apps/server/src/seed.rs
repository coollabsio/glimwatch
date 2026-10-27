use statixo_domain::{IncidentImpact, IncidentStatus, JobType, MonitorState};
use statixo_platform::{Id, TimestampMillis};

use crate::error::AppError;
use crate::repo::{self, Channel, Incident, IncidentUpdate, Maintenance, Monitor, Page};
use crate::state::{self, AppState};

pub const DEMO_EMAIL: &str = "test@example.com";
pub const DEMO_PASSWORD: &str = "password";

pub async fn seed(state: &AppState) -> Result<bool, AppError> {
    if repo::count_users(&state.db).await? > 0 {
        return Ok(false);
    }
    let now = TimestampMillis::now();
    let hash = state::hash_password(&state.passwords, DEMO_PASSWORD.to_owned())
        .await
        .map_err(|_| AppError::Internal)?;
    let user_id = Id::new();
    repo::insert_user(
        &state.db,
        user_id,
        DEMO_EMAIL,
        DEMO_EMAIL,
        "Test User",
        &hash,
        "owner",
        now,
    )
    .await?;
    crate::setup::mark_initialized(&state.db, user_id, now).await?;

    let origin = state.config.public_origin.trim_end_matches('/');
    let api = monitor(
        "Coolify API",
        "Public API and health endpoint for the Coolify control plane.",
        JobType::Http,
        &format!("{origin}/api/health"),
        "GET",
        10_000,
        20,
        Some(800),
        3,
        MonitorState::Up,
        0,
        now,
    );
    let database = monitor(
        "PostgreSQL",
        "Primary database accept path. A TCP connect is enough to call it reachable.",
        JobType::Tcp,
        &format!("127.0.0.1:{}", state.config.port),
        "GET",
        5_000,
        20,
        None,
        3,
        MonitorState::Up,
        0,
        now,
    );
    let worker = monitor(
        "Webhook worker",
        "Background worker that delivers outbound hooks.",
        JobType::Http,
        "http://127.0.0.1:9/health",
        "GET",
        3_000,
        20,
        None,
        2,
        MonitorState::Down,
        6,
        now,
    );
    for item in [&api, &database, &worker] {
        repo::insert_monitor(&state.db, item).await?;
    }

    let today = now.unix_day();
    fill_days(&state, api.id, today, DayPattern::Healthy).await?;
    fill_days(&state, database.id, today, DayPattern::Healthy).await?;
    fill_days(&state, worker.id, today, DayPattern::RecentOutage).await?;
    fill_recent(&state, &api, today, true).await?;
    fill_recent(&state, &database, today, true).await?;
    fill_recent(&state, &worker, today, false).await?;

    let page = Page {
        id: Id::new(),
        title: "coolLabs".to_owned(),
        description: "Status of Coolify and the services behind it.".to_owned(),
        slug: "coollabs".to_owned(),
        published: true,
        homepage_url: Some("https://coolify.io".to_owned()),
        created_at: now,
        updated_at: now,
    };
    repo::insert_page(&state.db, &page).await?;
    repo::replace_components(
        &state.db,
        page.id,
        &[
            (api.id, api.name.clone()),
            (database.id, database.name.clone()),
            (worker.id, worker.name.clone()),
        ],
        now,
    )
    .await?;

    let resolved_at = TimestampMillis::from_millis(now.as_millis() - 12 * 86_400_000);
    insert_history(&state, &api, page.id, resolved_at).await?;
    insert_open(&state, &worker, page.id, now).await?;

    let maintenance = Maintenance {
        id: Id::new(),
        page_id: Some(page.id),
        title: "Database failover drill".to_owned(),
        message: "We will fail the primary over to the replica and watch replication catch up. Reads stay up; writes pause for a few minutes.".to_owned(),
        starts_at: TimestampMillis::from_millis(now.as_millis() + 26 * 3_600_000),
        ends_at: TimestampMillis::from_millis(now.as_millis() + 28 * 3_600_000),
        monitor_ids: vec![database.id],
        created_at: now,
    };
    repo::insert_maintenance(&state.db, &maintenance).await?;

    let channel = Channel {
        id: Id::new(),
        name: "Ops email".to_owned(),
        provider: "email".to_owned(),
        config_json: format!(r#"{{"to":"{DEMO_EMAIL}"}}"#),
        enabled: true,
        created_at: now,
    };
    repo::insert_channel(&state.db, &channel, &[]).await?;
    crate::insights::seed_demo(state).await?;
    crate::errors::seed_demo(state).await?;
    let _ = repo::insert_audit(
        &state.db,
        Some(user_id),
        "seed.created",
        "installation",
        None,
        now,
    )
    .await;
    Ok(true)
}

fn monitor(
    name: &str,
    description: &str,
    job_type: JobType,
    url: &str,
    method: &str,
    timeout_ms: i64,
    interval_seconds: i64,
    degraded_after_ms: Option<i64>,
    failure_threshold: i32,
    state: MonitorState,
    consecutive_failures: i32,
    now: TimestampMillis,
) -> Monitor {
    Monitor {
        id: Id::new(),
        name: name.to_owned(),
        description: description.to_owned(),
        job_type,
        url: url.to_owned(),
        method: method.to_owned(),
        headers: Vec::new(),
        body: String::new(),
        timeout_ms,
        interval_seconds,
        degraded_after_ms,
        expected_status: None,
        assertions: Vec::new(),
        follow_redirects: true,
        failure_threshold,
        active: true,
        state,
        consecutive_failures,
        last_checked_at: Some(now),
        next_check_at: now,
        created_at: TimestampMillis::from_millis(now.as_millis() - 90 * 86_400_000),
        updated_at: now,
        agent_id: None,
        check_type: None,
    }
}

enum DayPattern {
    Healthy,
    RecentOutage,
}

async fn fill_days(
    state: &AppState,
    monitor_id: Id,
    today: i64,
    pattern: DayPattern,
) -> Result<(), AppError> {
    for offset in 0..90 {
        let day = today - 89 + offset;
        let age = today - day;
        let (success, degraded, failure, latency) = match pattern {
            DayPattern::Healthy if age == 12 => (40, 8, 0, 620),
            DayPattern::Healthy => (48, 0, 0, 42),
            DayPattern::RecentOutage if age <= 1 => (0, 0, 48, 12),
            DayPattern::RecentOutage if age == 6 => (20, 0, 28, 80),
            DayPattern::RecentOutage => (48, 0, 0, 18),
        };
        let count = success + degraded + failure;
        sqlx::query(
            "INSERT INTO monitor_daily_stats \
                (monitor_id, day, success, degraded, failure, maintenance, latency_sum, latency_count) \
             VALUES ($1, $2, $3, $4, $5, 0, $6, $7)",
        )
        .bind(monitor_id.to_string())
        .bind(day)
        .bind(success)
        .bind(degraded)
        .bind(failure)
        .bind(latency * count)
        .bind(count)
        .execute(state.db.pool())
        .await?;
    }
    Ok(())
}

async fn fill_recent(
    state: &AppState,
    monitor: &Monitor,
    today: i64,
    up: bool,
) -> Result<(), AppError> {
    let now = TimestampMillis::now().as_millis();
    for index in 0..36 {
        let checked_at = now - i64::from(index) * 60_000;
        let status = if up { "success" } else { "failure" };
        let latency = if up { 36 + i64::from(index % 9) * 4 } else { 8 };
        let error = if up { None } else { Some("connection refused") };
        sqlx::query(
            "INSERT INTO check_results (id, monitor_id, status, status_code, latency_ms, error, region, checked_at) \
             VALUES ($1, $2, $3, $4, $5, $6, 'local', $7)",
        )
        .bind(Id::new().to_string())
        .bind(monitor.id.to_string())
        .bind(status)
        .bind(if up { Some(200_i64) } else { None })
        .bind(latency)
        .bind(error)
        .bind(checked_at)
        .execute(state.db.pool())
        .await?;
    }
    let _ = today;
    Ok(())
}

async fn insert_history(
    state: &AppState,
    api: &Monitor,
    page_id: Id,
    when: TimestampMillis,
) -> Result<(), AppError> {
    let incident = Incident {
        id: Id::new(),
        page_id: Some(page_id),
        monitor_id: Some(api.id),
        title: "Elevated API latency".to_owned(),
        status: IncidentStatus::Resolved,
        impact: IncidentImpact::Minor,
        auto: false,
        started_at: when,
        resolved_at: Some(TimestampMillis::from_millis(
            when.as_millis() + 3 * 3_600_000,
        )),
        created_at: when,
        updated_at: TimestampMillis::from_millis(when.as_millis() + 3 * 3_600_000),
    };
    let first = IncidentUpdate {
        id: Id::new(),
        incident_id: incident.id,
        status: IncidentStatus::Investigating,
        message:
            "API latency climbed above the degraded line. We are looking at the database pool."
                .to_owned(),
        created_at: when,
    };
    let mut tx = state.db.begin().await?;
    repo::insert_incident(&mut tx, &incident, &first).await?;
    repo::insert_update(
        &mut tx,
        &IncidentUpdate {
            id: Id::new(),
            incident_id: incident.id,
            status: IncidentStatus::Identified,
            message:
                "A lock on the sessions table was holding checkouts. The query has been rewritten."
                    .to_owned(),
            created_at: TimestampMillis::from_millis(when.as_millis() + 45 * 60_000),
        },
    )
    .await?;
    repo::insert_update(
        &mut tx,
        &IncidentUpdate {
            id: Id::new(),
            incident_id: incident.id,
            status: IncidentStatus::Resolved,
            message: "Latency is back under 100ms. We are leaving the incident resolved."
                .to_owned(),
            created_at: TimestampMillis::from_millis(when.as_millis() + 3 * 3_600_000),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn insert_open(
    state: &AppState,
    worker: &Monitor,
    page_id: Id,
    now: TimestampMillis,
) -> Result<(), AppError> {
    let started = TimestampMillis::from_millis(now.as_millis() - 35 * 60_000);
    let incident = Incident {
        id: Id::new(),
        page_id: Some(page_id),
        monitor_id: Some(worker.id),
        title: "Webhook worker unreachable".to_owned(),
        status: IncidentStatus::Identified,
        impact: IncidentImpact::Major,
        auto: true,
        started_at: started,
        resolved_at: None,
        created_at: started,
        updated_at: TimestampMillis::from_millis(now.as_millis() - 10 * 60_000),
    };
    let first = IncidentUpdate {
        id: Id::new(),
        incident_id: incident.id,
        status: IncidentStatus::Investigating,
        message: "Checks to the worker are failing. We are looking at the host.".to_owned(),
        created_at: started,
    };
    let mut tx = state.db.begin().await?;
    repo::insert_incident(&mut tx, &incident, &first).await?;
    repo::insert_update(
        &mut tx,
        &IncidentUpdate {
            id: Id::new(),
            incident_id: incident.id,
            status: IncidentStatus::Identified,
            message: "The process is not accepting connections. A restart is in progress."
                .to_owned(),
            created_at: TimestampMillis::from_millis(now.as_millis() - 10 * 60_000),
        },
    )
    .await?;
    tx.commit().await?;
    Ok(())
}
