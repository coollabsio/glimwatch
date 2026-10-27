//! Persistence for both SQLite and Postgres. Statements use `$1` placeholders
//! and BIGINT counters so one query text runs on either backend.

use std::ops::DerefMut;

use haruvo_domain::{
    Assertion, CheckStatus, CheckType, DailyCount, HeaderPair, IncidentImpact, IncidentStatus,
    JobType, MonitorState,
};
use haruvo_platform::{Database, Id, ParseIdError, TimestampMillis};
use serde::de::DeserializeOwned;
use sqlx::{Any, Row, Transaction};

const MONITOR_COLUMNS: &str = "\
id, name, description, job_type, url, method, headers_json, body, timeout_ms, interval_seconds, \
degraded_after_ms, expected_status, assertions_json, follow_redirects, failure_threshold, active, \
state, consecutive_failures, last_checked_at, next_check_at, created_at, updated_at, agent_id, check_type";

#[derive(Clone, Debug)]
pub struct User {
    pub id: Id,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub password_hash: String,
    #[allow(dead_code)]
    pub created_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Monitor {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub job_type: JobType,
    pub url: String,
    pub method: String,
    pub headers: Vec<HeaderPair>,
    pub body: String,
    pub timeout_ms: i64,
    pub interval_seconds: i64,
    pub degraded_after_ms: Option<i64>,
    pub expected_status: Option<i64>,
    pub assertions: Vec<Assertion>,
    pub follow_redirects: bool,
    pub failure_threshold: i32,
    pub active: bool,
    pub state: MonitorState,
    pub consecutive_failures: i32,
    pub last_checked_at: Option<TimestampMillis>,
    pub next_check_at: TimestampMillis,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
    /// The agent that runs this check. `None` means this server runs it.
    pub agent_id: Option<Id>,
    /// Set only for agent checks. `None` means `job_type` says it all.
    pub check_type: Option<CheckType>,
}

#[derive(Clone, Debug)]
pub struct Agent {
    pub id: Id,
    pub name: String,
    pub last_seen_at: Option<TimestampMillis>,
    pub created_at: TimestampMillis,
    pub monitor_count: i64,
}

#[derive(Clone, Debug)]
pub struct NewCheck {
    pub id: Id,
    pub monitor_id: Id,
    pub status: CheckStatus,
    pub status_code: Option<i64>,
    pub latency_ms: i64,
    pub error: Option<String>,
    pub region: String,
    pub checked_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct CheckRow {
    pub id: Id,
    pub status: CheckStatus,
    pub status_code: Option<i64>,
    pub latency_ms: i64,
    pub error: Option<String>,
    pub region: String,
    pub checked_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct DailyRow {
    pub monitor_id: Id,
    pub count: DailyCount,
}

#[derive(Clone, Debug)]
pub struct StatusCount {
    pub monitor_id: Id,
    pub status: CheckStatus,
    pub total: i64,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub id: Id,
    pub title: String,
    pub description: String,
    pub slug: String,
    pub published: bool,
    pub homepage_url: Option<String>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Component {
    pub id: Id,
    #[allow(dead_code)]
    pub page_id: Id,
    pub monitor_id: Id,
    pub name: String,
    pub description: String,
    pub position: i64,
}

#[derive(Clone, Debug)]
pub struct Incident {
    pub id: Id,
    pub page_id: Option<Id>,
    pub monitor_id: Option<Id>,
    pub title: String,
    pub status: IncidentStatus,
    pub impact: IncidentImpact,
    pub auto: bool,
    pub started_at: TimestampMillis,
    pub resolved_at: Option<TimestampMillis>,
    pub created_at: TimestampMillis,
    pub updated_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct IncidentUpdate {
    pub id: Id,
    pub incident_id: Id,
    pub status: IncidentStatus,
    pub message: String,
    pub created_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Maintenance {
    pub id: Id,
    pub page_id: Option<Id>,
    pub title: String,
    pub message: String,
    pub starts_at: TimestampMillis,
    pub ends_at: TimestampMillis,
    pub monitor_ids: Vec<Id>,
    pub created_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Channel {
    pub id: Id,
    pub name: String,
    pub provider: String,
    pub config_json: String,
    pub enabled: bool,
    pub created_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Delivery {
    pub id: Id,
    #[allow(dead_code)]
    pub notification_id: Id,
    pub event: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: TimestampMillis,
}

#[derive(Clone, Debug)]
pub struct Audit {
    pub id: Id,
    #[allow(dead_code)]
    pub actor_id: Option<Id>,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub occurred_at: TimestampMillis,
}

pub async fn count_users(db: &Database) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM users")
        .fetch_one(db.pool())
        .await?;
    row.try_get("n")
}

pub async fn user_by_email(db: &Database, email: &str) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, email, display_name, role, password_hash, created_at FROM users WHERE normalized_email = $1",
    )
    .bind(email)
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_user).transpose()
}

#[derive(Clone, Debug)]
pub struct Account {
    pub id: Id,
    pub email: String,
    pub display_name: String,
    pub role: String,
}

pub async fn list_accounts(db: &Database) -> Result<Vec<Account>, sqlx::Error> {
    let rows =
        sqlx::query("SELECT id, email, display_name, role FROM users ORDER BY created_at, id")
            .fetch_all(db.pool())
            .await?;
    rows.iter()
        .map(|row| {
            Ok(Account {
                id: parse_id(row, "id")?,
                email: row.try_get("email")?,
                display_name: row.try_get("display_name")?,
                role: row.try_get("role")?,
            })
        })
        .collect()
}

pub async fn set_user_role(
    db: &Database,
    id: Id,
    role: &str,
    now: TimestampMillis,
) -> Result<bool, sqlx::Error> {
    let updated = sqlx::query("UPDATE users SET role = $1, updated_at = $2 WHERE id = $3")
        .bind(role)
        .bind(now.as_millis())
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(updated.rows_affected() == 1)
}

pub async fn insert_user(
    db: &Database,
    id: Id,
    email: &str,
    normalized: &str,
    display_name: &str,
    password_hash: &str,
    role: &str,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO users (id, email, normalized_email, display_name, password_hash, role, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
    )
    .bind(id.to_string())
    .bind(email)
    .bind(normalized)
    .bind(display_name)
    .bind(password_hash)
    .bind(role)
    .bind(now.as_millis())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn insert_session(
    db: &Database,
    id: Id,
    token_hash: &str,
    user_id: Id,
    now: TimestampMillis,
    idle_expires: TimestampMillis,
    absolute_expires: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, created_at, last_activity_at, idle_expires_at, absolute_expires_at) \
         VALUES ($1, $2, $3, $4, $4, $5, $6)",
    )
    .bind(id.to_string())
    .bind(token_hash)
    .bind(user_id.to_string())
    .bind(now.as_millis())
    .bind(idle_expires.as_millis())
    .bind(absolute_expires.as_millis())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn authenticate(
    db: &Database,
    token_hash: &str,
    now: TimestampMillis,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT user_id, last_activity_at, idle_expires_at, absolute_expires_at, revoked_at \
         FROM sessions WHERE token_hash = $1",
    )
    .bind(token_hash)
    .fetch_optional(db.pool())
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let revoked: Option<i64> = row.try_get("revoked_at")?;
    let idle: i64 = row.try_get("idle_expires_at")?;
    let absolute: i64 = row.try_get("absolute_expires_at")?;
    if revoked.is_some() || now.as_millis() >= idle || now.as_millis() >= absolute {
        return Ok(None);
    }
    let user_id: String = row.try_get("user_id")?;
    let last: i64 = row.try_get("last_activity_at")?;
    if now.as_millis() - last > 5 * 60 * 1000 {
        let idle_expires = now.as_millis() + 30 * 24 * 60 * 60 * 1000;
        sqlx::query(
            "UPDATE sessions SET last_activity_at = $1, idle_expires_at = $2 WHERE token_hash = $3",
        )
        .bind(now.as_millis())
        .bind(idle_expires.min(absolute))
        .bind(token_hash)
        .execute(db.pool())
        .await?;
    }
    let user = sqlx::query(
        "SELECT id, email, display_name, role, password_hash, created_at FROM users WHERE id = $1",
    )
    .bind(&user_id)
    .fetch_optional(db.pool())
    .await?;
    user.as_ref().map(map_user).transpose()
}

pub async fn revoke_token(
    db: &Database,
    token_hash: &str,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sessions SET revoked_at = $1 WHERE token_hash = $2 AND revoked_at IS NULL")
        .bind(now.as_millis())
        .bind(token_hash)
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn list_monitors(db: &Database) -> Result<Vec<Monitor>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "SELECT {MONITOR_COLUMNS} FROM monitors ORDER BY lower(name), id"
    ))
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_monitor).collect()
}

pub async fn get_monitor(db: &Database, id: Id) -> Result<Option<Monitor>, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {MONITOR_COLUMNS} FROM monitors WHERE id = $1"
    ))
    .bind(id.to_string())
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_monitor).transpose()
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_monitor(db: &Database, monitor: &Monitor) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO monitors (\
            id, name, description, job_type, url, method, headers_json, body, timeout_ms, interval_seconds, \
            degraded_after_ms, expected_status, assertions_json, follow_redirects, failure_threshold, active, \
            state, consecutive_failures, last_checked_at, next_check_at, created_at, updated_at, agent_id, check_type\
         ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24\
         )",
    )
    .bind(monitor.id.to_string())
    .bind(&monitor.name)
    .bind(&monitor.description)
    .bind(monitor.job_type.as_str())
    .bind(&monitor.url)
    .bind(&monitor.method)
    .bind(json(&monitor.headers))
    .bind(&monitor.body)
    .bind(monitor.timeout_ms)
    .bind(monitor.interval_seconds)
    .bind(monitor.degraded_after_ms)
    .bind(monitor.expected_status)
    .bind(json(&monitor.assertions))
    .bind(bit(monitor.follow_redirects))
    .bind(i64::from(monitor.failure_threshold))
    .bind(bit(monitor.active))
    .bind(monitor.state.as_str())
    .bind(i64::from(monitor.consecutive_failures))
    .bind(monitor.last_checked_at.map(TimestampMillis::as_millis))
    .bind(monitor.next_check_at.as_millis())
    .bind(monitor.created_at.as_millis())
    .bind(monitor.updated_at.as_millis())
    .bind(monitor.agent_id.map(|id| id.to_string()))
    .bind(monitor.check_type.map(CheckType::as_str))
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn update_monitor(db: &Database, monitor: &Monitor) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors SET name = $1, description = $2, job_type = $3, url = $4, method = $5, \
         headers_json = $6, body = $7, timeout_ms = $8, interval_seconds = $9, degraded_after_ms = $10, \
         expected_status = $11, assertions_json = $12, follow_redirects = $13, failure_threshold = $14, \
         active = $15, next_check_at = $16, updated_at = $17 WHERE id = $18",
    )
    .bind(&monitor.name)
    .bind(&monitor.description)
    .bind(monitor.job_type.as_str())
    .bind(&monitor.url)
    .bind(&monitor.method)
    .bind(json(&monitor.headers))
    .bind(&monitor.body)
    .bind(monitor.timeout_ms)
    .bind(monitor.interval_seconds)
    .bind(monitor.degraded_after_ms)
    .bind(monitor.expected_status)
    .bind(json(&monitor.assertions))
    .bind(bit(monitor.follow_redirects))
    .bind(i64::from(monitor.failure_threshold))
    .bind(bit(monitor.active))
    .bind(monitor.next_check_at.as_millis())
    .bind(monitor.updated_at.as_millis())
    .bind(monitor.id.to_string())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn set_active(
    db: &Database,
    id: Id,
    active: bool,
    next_check_at: i64,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors SET active = $1, next_check_at = $2, updated_at = $3 WHERE id = $4",
    )
    .bind(bit(active))
    .bind(next_check_at)
    .bind(now)
    .bind(id.to_string())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn delete_monitor(db: &Database, id: Id) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM monitors WHERE id = $1")
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn due_monitors(
    db: &Database,
    now: i64,
    limit: i64,
) -> Result<Vec<Monitor>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "SELECT {MONITOR_COLUMNS} FROM monitors \
         WHERE active = 1 AND agent_id IS NULL AND next_check_at <= $1 \
         ORDER BY next_check_at LIMIT $2"
    ))
    .bind(now)
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_monitor).collect()
}

pub async fn claim_monitor(
    db: &Database,
    id: Id,
    now: i64,
    next: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE monitors SET next_check_at = $1, updated_at = $2 \
         WHERE id = $3 AND active = 1 AND next_check_at <= $2",
    )
    .bind(next)
    .bind(now)
    .bind(id.to_string())
    .execute(db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

const AGENT_COLUMNS: &str = "\
a.id, a.name, a.last_seen_at, a.created_at, \
(SELECT COUNT(*) FROM monitors m WHERE m.agent_id = a.id) AS monitor_count";

pub async fn list_agents(db: &Database) -> Result<Vec<Agent>, sqlx::Error> {
    let rows = sqlx::query(&format!(
        "SELECT {AGENT_COLUMNS} FROM agents a ORDER BY lower(a.name), a.id"
    ))
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_agent).collect()
}

pub async fn get_agent(db: &Database, id: Id) -> Result<Option<Agent>, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {AGENT_COLUMNS} FROM agents a WHERE a.id = $1"
    ))
    .bind(id.to_string())
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_agent).transpose()
}

pub async fn agent_by_name(db: &Database, name: &str) -> Result<Option<Agent>, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {AGENT_COLUMNS} FROM agents a WHERE lower(a.name) = lower($1)"
    ))
    .bind(name)
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_agent).transpose()
}

pub async fn insert_agent(
    db: &Database,
    id: Id,
    name: &str,
    token_hash: &str,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO agents (id, name, token_hash, created_at) VALUES ($1, $2, $3, $4)")
        .bind(id.to_string())
        .bind(name)
        .bind(token_hash)
        .bind(now.as_millis())
        .execute(db.pool())
        .await?;
    Ok(())
}

/// What an agent reports about one check in its config. Secrets never reach the server.
#[derive(Clone, Debug)]
pub struct AgentCheck {
    pub name: String,
    pub check_type: CheckType,
    pub target: String,
    pub interval_seconds: i64,
    pub timeout_ms: i64,
    pub failure_threshold: i32,
}

/// Makes the agent's monitors match its config: listed checks are created or updated and
/// active, and the agent's other monitors are paused so their history stays.
pub async fn sync_agent_checks(
    db: &Database,
    agent_id: Id,
    checks: &[AgentCheck],
    now: TimestampMillis,
) -> Result<Vec<(String, Id)>, sqlx::Error> {
    let existing = sqlx::query(&format!(
        "SELECT {MONITOR_COLUMNS} FROM monitors WHERE agent_id = $1"
    ))
    .bind(agent_id.to_string())
    .fetch_all(db.pool())
    .await?
    .iter()
    .map(map_monitor)
    .collect::<Result<Vec<_>, _>>()?;
    let mut ids = Vec::new();
    for check in checks {
        let previous = existing.iter().find(|monitor| monitor.name == check.name);
        let monitor = Monitor {
            id: previous.map_or_else(Id::new, |monitor| monitor.id),
            name: check.name.clone(),
            description: String::new(),
            job_type: check.check_type.job_type(),
            url: check.target.clone(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: String::new(),
            timeout_ms: check.timeout_ms,
            interval_seconds: check.interval_seconds,
            degraded_after_ms: None,
            expected_status: None,
            assertions: Vec::new(),
            follow_redirects: true,
            failure_threshold: check.failure_threshold,
            active: true,
            state: previous.map_or(MonitorState::Pending, |monitor| monitor.state),
            consecutive_failures: previous.map_or(0, |monitor| monitor.consecutive_failures),
            last_checked_at: previous.and_then(|monitor| monitor.last_checked_at),
            next_check_at: now,
            created_at: previous.map_or(now, |monitor| monitor.created_at),
            updated_at: now,
            agent_id: Some(agent_id),
            check_type: Some(check.check_type),
        };
        if previous.is_some() {
            sqlx::query(
                "UPDATE monitors SET job_type = $1, check_type = $2, url = $3, timeout_ms = $4, \
                 interval_seconds = $5, failure_threshold = $6, active = 1, updated_at = $7 WHERE id = $8",
            )
            .bind(monitor.job_type.as_str())
            .bind(check.check_type.as_str())
            .bind(&monitor.url)
            .bind(monitor.timeout_ms)
            .bind(monitor.interval_seconds)
            .bind(i64::from(monitor.failure_threshold))
            .bind(now.as_millis())
            .bind(monitor.id.to_string())
            .execute(db.pool())
            .await?;
        } else {
            insert_monitor(db, &monitor).await?;
        }
        ids.push((monitor.name, monitor.id));
    }
    for monitor in &existing {
        if monitor.active && !checks.iter().any(|check| check.name == monitor.name) {
            set_active(
                db,
                monitor.id,
                false,
                monitor.next_check_at.as_millis(),
                now.as_millis(),
            )
            .await?;
        }
    }
    Ok(ids)
}

/// Deletes an agent that no monitor uses. Returns false when it is missing or in use.
pub async fn delete_agent(db: &Database, id: Id) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM agents WHERE id = $1 \
         AND NOT EXISTS (SELECT 1 FROM monitors WHERE agent_id = $1)",
    )
    .bind(id.to_string())
    .execute(db.pool())
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Finds the agent for a token and records that it was seen.
pub async fn authenticate_agent(
    db: &Database,
    token_hash: &str,
    now: TimestampMillis,
) -> Result<Option<Agent>, sqlx::Error> {
    let row = sqlx::query(&format!(
        "SELECT {AGENT_COLUMNS} FROM agents a WHERE a.token_hash = $1"
    ))
    .bind(token_hash)
    .fetch_optional(db.pool())
    .await?;
    let Some(agent) = row.as_ref().map(map_agent).transpose()? else {
        return Ok(None);
    };
    sqlx::query("UPDATE agents SET last_seen_at = $1 WHERE id = $2")
        .bind(now.as_millis())
        .bind(agent.id.to_string())
        .execute(db.pool())
        .await?;
    Ok(Some(agent))
}

pub async fn save_outcome(
    tx: &mut Transaction<'_, Any>,
    id: Id,
    state: MonitorState,
    consecutive_failures: i32,
    checked_at: i64,
    next_check_at: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE monitors SET state = $1, consecutive_failures = $2, last_checked_at = $3, \
         next_check_at = $4, updated_at = $3 WHERE id = $5",
    )
    .bind(state.as_str())
    .bind(i64::from(consecutive_failures))
    .bind(checked_at)
    .bind(next_check_at)
    .bind(id.to_string())
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

pub async fn insert_check(
    tx: &mut Transaction<'_, Any>,
    check: &NewCheck,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO check_results (id, monitor_id, status, status_code, latency_ms, error, region, checked_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(check.id.to_string())
    .bind(check.monitor_id.to_string())
    .bind(check.status.as_str())
    .bind(check.status_code)
    .bind(check.latency_ms)
    .bind(&check.error)
    .bind(&check.region)
    .bind(check.checked_at.as_millis())
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

pub async fn bump_daily(
    tx: &mut Transaction<'_, Any>,
    monitor_id: Id,
    day: i64,
    status: CheckStatus,
    latency_ms: i64,
) -> Result<(), sqlx::Error> {
    let (success, degraded, failure, maintenance, latency_sum, latency_count) = match status {
        CheckStatus::Success => (1, 0, 0, 0, latency_ms, 1),
        CheckStatus::Degraded => (0, 1, 0, 0, latency_ms, 1),
        CheckStatus::Failure => (0, 0, 1, 0, latency_ms, 1),
        CheckStatus::Maintenance => (0, 0, 0, 1, 0, 0),
    };
    sqlx::query(
        "INSERT INTO monitor_daily_stats \
            (monitor_id, day, success, degraded, failure, maintenance, latency_sum, latency_count) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
         ON CONFLICT (monitor_id, day) DO UPDATE SET \
            success = monitor_daily_stats.success + EXCLUDED.success, \
            degraded = monitor_daily_stats.degraded + EXCLUDED.degraded, \
            failure = monitor_daily_stats.failure + EXCLUDED.failure, \
            maintenance = monitor_daily_stats.maintenance + EXCLUDED.maintenance, \
            latency_sum = monitor_daily_stats.latency_sum + EXCLUDED.latency_sum, \
            latency_count = monitor_daily_stats.latency_count + EXCLUDED.latency_count",
    )
    .bind(monitor_id.to_string())
    .bind(day)
    .bind(success)
    .bind(degraded)
    .bind(failure)
    .bind(maintenance)
    .bind(latency_sum)
    .bind(latency_count)
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

pub async fn recent_checks(
    db: &Database,
    monitor_id: Id,
    limit: i64,
) -> Result<Vec<CheckRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, status, status_code, latency_ms, error, region, checked_at FROM check_results \
         WHERE monitor_id = $1 ORDER BY checked_at DESC LIMIT $2",
    )
    .bind(monitor_id.to_string())
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(CheckRow {
                id: parse_id(row, "id")?,
                status: parse_status(&row.try_get::<String, _>("status")?)?,
                status_code: row.try_get("status_code")?,
                latency_ms: row.try_get("latency_ms")?,
                error: row.try_get("error")?,
                region: row.try_get("region")?,
                checked_at: millis(row, "checked_at")?,
            })
        })
        .collect()
}

pub async fn daily_since(db: &Database, start_day: i64) -> Result<Vec<DailyRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT monitor_id, day, success, degraded, failure, maintenance, latency_sum, latency_count \
         FROM monitor_daily_stats WHERE day >= $1",
    )
    .bind(start_day)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(DailyRow {
                monitor_id: parse_id(row, "monitor_id")?,
                count: DailyCount {
                    day: row.try_get("day")?,
                    success: row.try_get("success")?,
                    degraded: row.try_get("degraded")?,
                    failure: row.try_get("failure")?,
                    maintenance: row.try_get("maintenance")?,
                    latency_sum: row.try_get("latency_sum")?,
                    latency_count: row.try_get("latency_count")?,
                },
            })
        })
        .collect()
}

pub async fn counts_since(db: &Database, since: i64) -> Result<Vec<StatusCount>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT monitor_id, status, COUNT(*) AS n FROM check_results \
         WHERE checked_at >= $1 GROUP BY monitor_id, status",
    )
    .bind(since)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(StatusCount {
                monitor_id: parse_id(row, "monitor_id")?,
                status: parse_status(&row.try_get::<String, _>("status")?)?,
                total: row.try_get("n")?,
            })
        })
        .collect()
}

pub async fn prune(db: &Database, older_than: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM check_results WHERE checked_at < $1")
        .bind(older_than)
        .execute(db.pool())
        .await?;
    sqlx::query("DELETE FROM notification_deliveries WHERE created_at < $1")
        .bind(older_than)
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn list_pages(db: &Database) -> Result<Vec<Page>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, title, description, slug, published, homepage_url, created_at, updated_at \
         FROM status_pages ORDER BY lower(title)",
    )
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_page).collect()
}

pub async fn get_page(db: &Database, id: Id) -> Result<Option<Page>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, title, description, slug, published, homepage_url, created_at, updated_at \
         FROM status_pages WHERE id = $1",
    )
    .bind(id.to_string())
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_page).transpose()
}

pub async fn page_by_slug(db: &Database, slug: &str) -> Result<Option<Page>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, title, description, slug, published, homepage_url, created_at, updated_at \
         FROM status_pages WHERE slug = $1",
    )
    .bind(slug)
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_page).transpose()
}

pub async fn insert_page(db: &Database, page: &Page) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO status_pages (id, title, description, slug, published, homepage_url, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(page.id.to_string())
    .bind(&page.title)
    .bind(&page.description)
    .bind(&page.slug)
    .bind(bit(page.published))
    .bind(&page.homepage_url)
    .bind(page.created_at.as_millis())
    .bind(page.updated_at.as_millis())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn update_page(db: &Database, page: &Page) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE status_pages SET title = $1, description = $2, slug = $3, published = $4, \
         homepage_url = $5, updated_at = $6 WHERE id = $7",
    )
    .bind(&page.title)
    .bind(&page.description)
    .bind(&page.slug)
    .bind(bit(page.published))
    .bind(&page.homepage_url)
    .bind(page.updated_at.as_millis())
    .bind(page.id.to_string())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn delete_page(db: &Database, id: Id) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM status_pages WHERE id = $1")
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn components(db: &Database, page_id: Id) -> Result<Vec<Component>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, page_id, monitor_id, name, description, position FROM page_components \
         WHERE page_id = $1 ORDER BY position, name",
    )
    .bind(page_id.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(Component {
                id: parse_id(row, "id")?,
                page_id: parse_id(row, "page_id")?,
                monitor_id: parse_id(row, "monitor_id")?,
                name: row.try_get("name")?,
                description: row.try_get("description")?,
                position: row.try_get("position")?,
            })
        })
        .collect()
}

pub async fn replace_components(
    db: &Database,
    page_id: Id,
    monitors: &[(Id, String)],
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM page_components WHERE page_id = $1")
        .bind(page_id.to_string())
        .execute(tx.deref_mut())
        .await?;
    for (position, (monitor_id, name)) in monitors.iter().enumerate() {
        sqlx::query(
            "INSERT INTO page_components (id, page_id, monitor_id, name, description, position, created_at) \
             VALUES ($1, $2, $3, $4, '', $5, $6)",
        )
        .bind(Id::new().to_string())
        .bind(page_id.to_string())
        .bind(monitor_id.to_string())
        .bind(name)
        .bind(position as i64)
        .bind(now.as_millis())
        .execute(tx.deref_mut())
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn list_incidents(db: &Database) -> Result<Vec<Incident>, sqlx::Error> {
    let rows = sqlx::query(&incident_select("1 = 1"))
        .fetch_all(db.pool())
        .await?;
    rows.iter().map(map_incident).collect()
}

pub async fn get_incident(db: &Database, id: Id) -> Result<Option<Incident>, sqlx::Error> {
    let row = sqlx::query(&incident_select("id = $1"))
        .bind(id.to_string())
        .fetch_optional(db.pool())
        .await?;
    row.as_ref().map(map_incident).transpose()
}

pub async fn incidents_for_page(db: &Database, page_id: Id) -> Result<Vec<Incident>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, page_id, monitor_id, title, status, impact, auto, started_at, resolved_at, created_at, updated_at \
         FROM incidents WHERE page_id = $1 OR monitor_id IN \
         (SELECT monitor_id FROM page_components WHERE page_id = $1) \
         ORDER BY started_at DESC LIMIT 40",
    )
    .bind(page_id.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_incident).collect()
}

pub async fn updates_for(
    db: &Database,
    incident_ids: &[Id],
) -> Result<Vec<IncidentUpdate>, sqlx::Error> {
    if incident_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = (1..=incident_ids.len())
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, incident_id, status, message, created_at FROM incident_updates \
         WHERE incident_id IN ({placeholders}) ORDER BY created_at"
    );
    let mut query = sqlx::query(&sql);
    for id in incident_ids {
        query = query.bind(id.to_string());
    }
    let rows = query.fetch_all(db.pool()).await?;
    rows.iter()
        .map(|row| {
            Ok(IncidentUpdate {
                id: parse_id(row, "id")?,
                incident_id: parse_id(row, "incident_id")?,
                status: parse_incident_status(&row.try_get::<String, _>("status")?)?,
                message: row.try_get("message")?,
                created_at: millis(row, "created_at")?,
            })
        })
        .collect()
}

pub async fn insert_incident(
    tx: &mut Transaction<'_, Any>,
    incident: &Incident,
    update: &IncidentUpdate,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO incidents (id, page_id, monitor_id, title, status, impact, auto, started_at, resolved_at, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
    )
    .bind(incident.id.to_string())
    .bind(incident.page_id.map(|id| id.to_string()))
    .bind(incident.monitor_id.map(|id| id.to_string()))
    .bind(&incident.title)
    .bind(incident.status.as_str())
    .bind(incident.impact.as_str())
    .bind(bit(incident.auto))
    .bind(incident.started_at.as_millis())
    .bind(incident.resolved_at.map(TimestampMillis::as_millis))
    .bind(incident.created_at.as_millis())
    .bind(incident.updated_at.as_millis())
    .execute(tx.deref_mut())
    .await?;
    insert_update(tx, update).await
}

pub async fn insert_update(
    tx: &mut Transaction<'_, Any>,
    update: &IncidentUpdate,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO incident_updates (id, incident_id, status, message, created_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(update.id.to_string())
    .bind(update.incident_id.to_string())
    .bind(update.status.as_str())
    .bind(&update.message)
    .bind(update.created_at.as_millis())
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

pub async fn apply_incident_status(
    tx: &mut Transaction<'_, Any>,
    id: Id,
    status: IncidentStatus,
    resolved_at: Option<i64>,
    now: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE incidents SET status = $1, resolved_at = $2, updated_at = $3 WHERE id = $4",
    )
    .bind(status.as_str())
    .bind(resolved_at)
    .bind(now)
    .bind(id.to_string())
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

pub async fn list_maintenances(db: &Database) -> Result<Vec<Maintenance>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, page_id, title, message, starts_at, ends_at, created_at FROM maintenances ORDER BY starts_at DESC",
    )
    .fetch_all(db.pool())
    .await?;
    let mut maintenances = Vec::new();
    for row in &rows {
        maintenances.push(map_maintenance(db, row).await?);
    }
    Ok(maintenances)
}

pub async fn maintenances_for_page(
    db: &Database,
    page_id: Id,
) -> Result<Vec<Maintenance>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, page_id, title, message, starts_at, ends_at, created_at FROM maintenances \
         WHERE page_id = $1 ORDER BY starts_at DESC LIMIT 20",
    )
    .bind(page_id.to_string())
    .fetch_all(db.pool())
    .await?;
    let mut maintenances = Vec::new();
    for row in &rows {
        maintenances.push(map_maintenance(db, row).await?);
    }
    Ok(maintenances)
}

pub async fn insert_maintenance(
    db: &Database,
    maintenance: &Maintenance,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO maintenances (id, page_id, title, message, starts_at, ends_at, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $7)",
    )
    .bind(maintenance.id.to_string())
    .bind(maintenance.page_id.map(|id| id.to_string()))
    .bind(&maintenance.title)
    .bind(&maintenance.message)
    .bind(maintenance.starts_at.as_millis())
    .bind(maintenance.ends_at.as_millis())
    .bind(maintenance.created_at.as_millis())
    .execute(tx.deref_mut())
    .await?;
    for monitor_id in &maintenance.monitor_ids {
        sqlx::query(
            "INSERT INTO maintenance_monitors (maintenance_id, monitor_id) VALUES ($1, $2)",
        )
        .bind(maintenance.id.to_string())
        .bind(monitor_id.to_string())
        .execute(tx.deref_mut())
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_maintenance(db: &Database, id: Id) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM maintenances WHERE id = $1")
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn in_maintenance(db: &Database, monitor_id: Id, now: i64) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COUNT(*) AS n FROM maintenances m \
         WHERE m.starts_at <= $1 AND m.ends_at >= $1 AND ( \
            EXISTS (SELECT 1 FROM maintenance_monitors mm WHERE mm.maintenance_id = m.id AND mm.monitor_id = $2) \
            OR ( \
                NOT EXISTS (SELECT 1 FROM maintenance_monitors mm WHERE mm.maintenance_id = m.id) \
                AND m.page_id IS NOT NULL \
                AND EXISTS (SELECT 1 FROM page_components pc WHERE pc.page_id = m.page_id AND pc.monitor_id = $2) \
            ) \
         )",
    )
    .bind(now)
    .bind(monitor_id.to_string())
    .fetch_one(db.pool())
    .await?;
    let count: i64 = row.try_get("n")?;
    Ok(count > 0)
}

pub async fn list_channels(db: &Database) -> Result<Vec<Channel>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, name, provider, config_json, enabled, created_at FROM notifications ORDER BY lower(name)",
    )
    .fetch_all(db.pool())
    .await?;
    rows.iter().map(map_channel).collect()
}

pub async fn get_channel(db: &Database, id: Id) -> Result<Option<Channel>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, name, provider, config_json, enabled, created_at FROM notifications WHERE id = $1",
    )
    .bind(id.to_string())
    .fetch_optional(db.pool())
    .await?;
    row.as_ref().map(map_channel).transpose()
}

pub async fn channel_monitors(db: &Database, id: Id) -> Result<Vec<Id>, sqlx::Error> {
    let rows =
        sqlx::query("SELECT monitor_id FROM notification_monitors WHERE notification_id = $1")
            .bind(id.to_string())
            .fetch_all(db.pool())
            .await?;
    rows.iter().map(|row| parse_id(row, "monitor_id")).collect()
}

pub async fn insert_channel(
    db: &Database,
    channel: &Channel,
    monitor_ids: &[Id],
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO notifications (id, name, provider, config_json, enabled, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
    )
    .bind(channel.id.to_string())
    .bind(&channel.name)
    .bind(&channel.provider)
    .bind(&channel.config_json)
    .bind(bit(channel.enabled))
    .bind(channel.created_at.as_millis())
    .execute(tx.deref_mut())
    .await?;
    link_channel_monitors(&mut tx, channel.id, monitor_ids).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_channel(
    db: &Database,
    channel: &Channel,
    monitor_ids: &[Id],
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE notifications SET name = $1, provider = $2, config_json = $3, enabled = $4, updated_at = $5 WHERE id = $6",
    )
    .bind(&channel.name)
    .bind(&channel.provider)
    .bind(&channel.config_json)
    .bind(bit(channel.enabled))
    .bind(now.as_millis())
    .bind(channel.id.to_string())
    .execute(tx.deref_mut())
    .await?;
    sqlx::query("DELETE FROM notification_monitors WHERE notification_id = $1")
        .bind(channel.id.to_string())
        .execute(tx.deref_mut())
        .await?;
    link_channel_monitors(&mut tx, channel.id, monitor_ids).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_channel(db: &Database, id: Id) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("DELETE FROM notifications WHERE id = $1")
        .bind(id.to_string())
        .execute(db.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn channels_for_monitor(
    db: &Database,
    monitor_id: Option<Id>,
) -> Result<Vec<Channel>, sqlx::Error> {
    let rows = match monitor_id {
        Some(monitor_id) => {
            sqlx::query(
                "SELECT id, name, provider, config_json, enabled, created_at FROM notifications n \
                 WHERE enabled = 1 AND ( \
                    NOT EXISTS (SELECT 1 FROM notification_monitors nm WHERE nm.notification_id = n.id) \
                    OR EXISTS (SELECT 1 FROM notification_monitors nm WHERE nm.notification_id = n.id AND nm.monitor_id = $1) \
                 )",
            )
            .bind(monitor_id.to_string())
            .fetch_all(db.pool())
            .await?
        }
        None => {
            sqlx::query(
                "SELECT id, name, provider, config_json, enabled, created_at FROM notifications n \
                 WHERE enabled = 1 AND NOT EXISTS \
                 (SELECT 1 FROM notification_monitors nm WHERE nm.notification_id = n.id)",
            )
            .fetch_all(db.pool())
            .await?
        }
    };
    rows.iter().map(map_channel).collect()
}

pub async fn insert_delivery(
    db: &Database,
    id: Id,
    notification_id: Id,
    monitor_id: Option<Id>,
    incident_id: Option<Id>,
    event: &str,
    status: &str,
    error: Option<&str>,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO notification_deliveries \
            (id, notification_id, monitor_id, incident_id, event, status, error, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id.to_string())
    .bind(notification_id.to_string())
    .bind(monitor_id.map(|id| id.to_string()))
    .bind(incident_id.map(|id| id.to_string()))
    .bind(event)
    .bind(status)
    .bind(error)
    .bind(now.as_millis())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn recent_deliveries(
    db: &Database,
    notification_id: Id,
) -> Result<Vec<Delivery>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, notification_id, event, status, error, created_at FROM notification_deliveries \
         WHERE notification_id = $1 ORDER BY created_at DESC LIMIT 8",
    )
    .bind(notification_id.to_string())
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(Delivery {
                id: parse_id(row, "id")?,
                notification_id: parse_id(row, "notification_id")?,
                event: row.try_get("event")?,
                status: row.try_get("status")?,
                error: row.try_get("error")?,
                created_at: millis(row, "created_at")?,
            })
        })
        .collect()
}

pub async fn insert_audit(
    db: &Database,
    actor_id: Option<Id>,
    action: &str,
    resource_type: &str,
    resource_id: Option<&str>,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, action, resource_type, resource_id, metadata_json, occurred_at) \
         VALUES ($1, $2, $3, $4, $5, '{}', $6)",
    )
    .bind(Id::new().to_string())
    .bind(actor_id.map(|id| id.to_string()))
    .bind(action)
    .bind(resource_type)
    .bind(resource_id)
    .bind(now.as_millis())
    .execute(db.pool())
    .await?;
    Ok(())
}

pub async fn recent_audit(db: &Database, limit: i64) -> Result<Vec<Audit>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, actor_id, action, resource_type, resource_id, occurred_at FROM audit_events \
         ORDER BY occurred_at DESC LIMIT $1",
    )
    .bind(limit)
    .fetch_all(db.pool())
    .await?;
    rows.iter()
        .map(|row| {
            Ok(Audit {
                id: parse_id(row, "id")?,
                actor_id: optional_id(row, "actor_id")?,
                action: row.try_get("action")?,
                resource_type: row.try_get("resource_type")?,
                resource_id: row.try_get("resource_id")?,
                occurred_at: millis(row, "occurred_at")?,
            })
        })
        .collect()
}

pub async fn try_scheduler_lock(db: &Database, owner: &str, now: i64) -> Result<bool, sqlx::Error> {
    let expires = now + 30_000;
    let row = sqlx::query(
        "INSERT INTO scheduler_lock (id, owner, expires_at) VALUES (1, $1, $2) \
         ON CONFLICT (id) DO UPDATE SET owner = EXCLUDED.owner, expires_at = EXCLUDED.expires_at \
         WHERE scheduler_lock.expires_at < $3 OR scheduler_lock.owner = $1 \
         RETURNING owner",
    )
    .bind(owner)
    .bind(expires)
    .bind(now)
    .fetch_optional(db.pool())
    .await?;
    Ok(row.is_some())
}

async fn link_channel_monitors(
    tx: &mut Transaction<'_, Any>,
    channel_id: Id,
    monitor_ids: &[Id],
) -> Result<(), sqlx::Error> {
    for monitor_id in monitor_ids {
        sqlx::query(
            "INSERT INTO notification_monitors (notification_id, monitor_id) VALUES ($1, $2)",
        )
        .bind(channel_id.to_string())
        .bind(monitor_id.to_string())
        .execute(tx.deref_mut())
        .await?;
    }
    Ok(())
}

fn incident_select(filter: &str) -> String {
    format!(
        "SELECT id, page_id, monitor_id, title, status, impact, auto, started_at, resolved_at, created_at, updated_at \
         FROM incidents WHERE {filter} ORDER BY started_at DESC LIMIT 100"
    )
}

async fn map_maintenance(
    db: &Database,
    row: &sqlx::any::AnyRow,
) -> Result<Maintenance, sqlx::Error> {
    let id = parse_id(row, "id")?;
    let links =
        sqlx::query("SELECT monitor_id FROM maintenance_monitors WHERE maintenance_id = $1")
            .bind(id.to_string())
            .fetch_all(db.pool())
            .await?;
    let monitor_ids = links
        .iter()
        .map(|link| parse_id(link, "monitor_id"))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Maintenance {
        id,
        page_id: optional_id(row, "page_id")?,
        title: row.try_get("title")?,
        message: row.try_get("message")?,
        starts_at: millis(row, "starts_at")?,
        ends_at: millis(row, "ends_at")?,
        monitor_ids,
        created_at: millis(row, "created_at")?,
    })
}

fn map_user(row: &sqlx::any::AnyRow) -> Result<User, sqlx::Error> {
    Ok(User {
        id: parse_id(row, "id")?,
        email: row.try_get("email")?,
        display_name: row.try_get("display_name")?,
        role: row.try_get("role")?,
        password_hash: row.try_get("password_hash")?,
        created_at: millis(row, "created_at")?,
    })
}

fn map_monitor(row: &sqlx::any::AnyRow) -> Result<Monitor, sqlx::Error> {
    let job_type = row.try_get::<String, _>("job_type")?;
    let state = row.try_get::<String, _>("state")?;
    Ok(Monitor {
        id: parse_id(row, "id")?,
        name: row.try_get("name")?,
        description: row.try_get("description")?,
        job_type: JobType::parse(&job_type)
            .ok_or_else(|| corrupt(format!("job type {job_type}")))?,
        url: row.try_get("url")?,
        method: row.try_get("method")?,
        headers: parse_json(&row.try_get::<String, _>("headers_json")?),
        body: row.try_get("body")?,
        timeout_ms: row.try_get("timeout_ms")?,
        interval_seconds: row.try_get("interval_seconds")?,
        degraded_after_ms: row.try_get("degraded_after_ms")?,
        expected_status: row.try_get("expected_status")?,
        assertions: parse_json(&row.try_get::<String, _>("assertions_json")?),
        follow_redirects: flag(row.try_get("follow_redirects")?),
        failure_threshold: i32::try_from(row.try_get::<i64, _>("failure_threshold")?).unwrap_or(3),
        active: flag(row.try_get("active")?),
        state: MonitorState::parse(&state).ok_or_else(|| corrupt(format!("state {state}")))?,
        consecutive_failures: i32::try_from(row.try_get::<i64, _>("consecutive_failures")?)
            .unwrap_or(0),
        last_checked_at: optional_millis(row, "last_checked_at")?,
        next_check_at: millis(row, "next_check_at")?,
        created_at: millis(row, "created_at")?,
        updated_at: millis(row, "updated_at")?,
        agent_id: optional_id(row, "agent_id")?,
        check_type: row
            .try_get::<Option<String>, _>("check_type")?
            .map(|value| {
                CheckType::parse(&value).ok_or_else(|| corrupt(format!("check type {value}")))
            })
            .transpose()?,
    })
}

fn map_agent(row: &sqlx::any::AnyRow) -> Result<Agent, sqlx::Error> {
    Ok(Agent {
        id: parse_id(row, "id")?,
        name: row.try_get("name")?,
        last_seen_at: optional_millis(row, "last_seen_at")?,
        created_at: millis(row, "created_at")?,
        monitor_count: row.try_get("monitor_count")?,
    })
}

fn map_page(row: &sqlx::any::AnyRow) -> Result<Page, sqlx::Error> {
    Ok(Page {
        id: parse_id(row, "id")?,
        title: row.try_get("title")?,
        description: row.try_get("description")?,
        slug: row.try_get("slug")?,
        published: flag(row.try_get("published")?),
        homepage_url: row.try_get("homepage_url")?,
        created_at: millis(row, "created_at")?,
        updated_at: millis(row, "updated_at")?,
    })
}

fn map_incident(row: &sqlx::any::AnyRow) -> Result<Incident, sqlx::Error> {
    let status = row.try_get::<String, _>("status")?;
    let impact = row.try_get::<String, _>("impact")?;
    Ok(Incident {
        id: parse_id(row, "id")?,
        page_id: optional_id(row, "page_id")?,
        monitor_id: optional_id(row, "monitor_id")?,
        title: row.try_get("title")?,
        status: parse_incident_status(&status)?,
        impact: IncidentImpact::parse(&impact)
            .ok_or_else(|| corrupt(format!("impact {impact}")))?,
        auto: flag(row.try_get("auto")?),
        started_at: millis(row, "started_at")?,
        resolved_at: optional_millis(row, "resolved_at")?,
        created_at: millis(row, "created_at")?,
        updated_at: millis(row, "updated_at")?,
    })
}

fn map_channel(row: &sqlx::any::AnyRow) -> Result<Channel, sqlx::Error> {
    Ok(Channel {
        id: parse_id(row, "id")?,
        name: row.try_get("name")?,
        provider: row.try_get("provider")?,
        config_json: row.try_get("config_json")?,
        enabled: flag(row.try_get("enabled")?),
        created_at: millis(row, "created_at")?,
    })
}

fn parse_status(value: &str) -> Result<CheckStatus, sqlx::Error> {
    CheckStatus::parse(value).ok_or_else(|| corrupt(format!("check status {value}")))
}

fn parse_incident_status(value: &str) -> Result<IncidentStatus, sqlx::Error> {
    IncidentStatus::parse(value).ok_or_else(|| corrupt(format!("incident status {value}")))
}

fn parse_id(row: &sqlx::any::AnyRow, name: &str) -> Result<Id, sqlx::Error> {
    let value: String = row.try_get(name)?;
    value
        .parse()
        .map_err(|error: ParseIdError| corrupt(error.to_string()))
}

fn optional_id(row: &sqlx::any::AnyRow, name: &str) -> Result<Option<Id>, sqlx::Error> {
    let value: Option<String> = row.try_get(name)?;
    value
        .map(|value| {
            value
                .parse()
                .map_err(|error: ParseIdError| corrupt(error.to_string()))
        })
        .transpose()
}

fn millis(row: &sqlx::any::AnyRow, name: &str) -> Result<TimestampMillis, sqlx::Error> {
    Ok(TimestampMillis::from_millis(row.try_get(name)?))
}

fn optional_millis(
    row: &sqlx::any::AnyRow,
    name: &str,
) -> Result<Option<TimestampMillis>, sqlx::Error> {
    let value: Option<i64> = row.try_get(name)?;
    Ok(value.map(TimestampMillis::from_millis))
}

fn flag(value: i64) -> bool {
    value != 0
}

fn bit(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_owned())
}

fn parse_json<T: DeserializeOwned + Default>(value: &str) -> T {
    serde_json::from_str(value).unwrap_or_default()
}

fn corrupt(message: String) -> sqlx::Error {
    sqlx::Error::Protocol(message)
}
