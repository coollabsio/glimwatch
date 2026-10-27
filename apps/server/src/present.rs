use chrono::{DateTime, Utc};
use haruvo_domain::{
    Assertion, CheckStatus, CheckType, DailyCount, HeaderPair, IncidentImpact, IncidentStatus,
    JobType, MonitorState, PageBanner, summarize,
};
use haruvo_platform::{Id, TimestampMillis};
use serde::Serialize;
use serde_json::Value;

use crate::repo::{
    self, Audit, Channel, CheckRow, Component, DailyRow, Delivery, Incident, IncidentUpdate,
    Maintenance, Monitor, Page, StatusCount, User,
};

#[derive(Serialize)]
pub struct UserResponse {
    pub id: Id,
    pub email: String,
    pub display_name: String,
    pub role: String,
    /// The account created at setup. It cannot be demoted.
    pub root: bool,
    /// Root, or an account the root user promoted. Can view the user list.
    pub installation_admin: bool,
}

impl UserResponse {
    #[must_use]
    pub fn with_access(user: &User, root_user_id: Option<Id>) -> Self {
        let root = root_user_id == Some(user.id);
        Self {
            id: user.id,
            email: user.email.clone(),
            display_name: user.display_name.clone(),
            role: user.role.clone(),
            root,
            installation_admin: root || user.role == "owner" || user.role == "admin",
        }
    }
}

#[derive(Serialize)]
pub struct BarResponse {
    pub day: String,
    pub status: String,
    pub uptime_ratio: Option<f64>,
    pub total: i64,
    pub avg_latency_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct CheckResponse {
    pub id: Id,
    pub status: CheckStatus,
    pub status_code: Option<i64>,
    pub latency_ms: i64,
    pub error: Option<String>,
    /// `local` for this server, or the name of the agent that ran the check.
    pub region: String,
    pub checked_at: TimestampMillis,
}

#[derive(Serialize)]
pub struct MonitorResponse {
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
    pub agent_id: Option<Id>,
    pub check_type: CheckType,
    pub uptime_24h: Option<f64>,
    pub uptime_30d: Option<f64>,
    pub uptime_90d: Option<f64>,
    pub avg_latency_ms: Option<i64>,
    pub bars: Vec<BarResponse>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent: Vec<CheckResponse>,
}

pub struct Stats<'a> {
    pub daily: &'a [DailyRow],
    pub counts: &'a [StatusCount],
}

pub fn monitor_response(
    monitor: &Monitor,
    stats: Stats<'_>,
    bar_days: i64,
    recent: Vec<CheckRow>,
) -> MonitorResponse {
    let today = TimestampMillis::now().unix_day();
    let mine = stats
        .daily
        .iter()
        .filter(|row| row.monitor_id == monitor.id)
        .map(|row| row.count.clone())
        .collect::<Vec<_>>();
    let long = summarize(&mine, today, 90);
    let mid = summarize(&mine, today, 30);
    let bars = summarize(&mine, today, bar_days);
    let today_bucket = mine.iter().find(|count| count.day == today);
    MonitorResponse {
        id: monitor.id,
        name: monitor.name.clone(),
        description: monitor.description.clone(),
        job_type: monitor.job_type,
        url: monitor.url.clone(),
        method: monitor.method.clone(),
        headers: monitor.headers.clone(),
        body: monitor.body.clone(),
        timeout_ms: monitor.timeout_ms,
        interval_seconds: monitor.interval_seconds,
        degraded_after_ms: monitor.degraded_after_ms,
        expected_status: monitor.expected_status,
        assertions: monitor.assertions.clone(),
        follow_redirects: monitor.follow_redirects,
        failure_threshold: monitor.failure_threshold,
        active: monitor.active,
        state: monitor.state,
        consecutive_failures: monitor.consecutive_failures,
        last_checked_at: monitor.last_checked_at,
        next_check_at: monitor.next_check_at,
        created_at: monitor.created_at,
        agent_id: monitor.agent_id,
        check_type: monitor.check_type.unwrap_or(match monitor.job_type {
            JobType::Http => CheckType::Http,
            JobType::Tcp => CheckType::Tcp,
        }),
        uptime_24h: uptime_24h(stats.counts, monitor.id).or(long_window_today(&mine, today)),
        uptime_30d: mid.uptime_ratio,
        uptime_90d: long.uptime_ratio,
        avg_latency_ms: today_bucket
            .and_then(|count| average(count.latency_sum, count.latency_count)),
        bars: bars.days.into_iter().map(bar).collect(),
        recent: recent.into_iter().map(check_response).collect(),
    }
}

fn uptime_24h(counts: &[StatusCount], id: Id) -> Option<f64> {
    let mut up = 0;
    let mut down = 0;
    for count in counts.iter().filter(|count| count.monitor_id == id) {
        match count.status {
            CheckStatus::Success | CheckStatus::Degraded => up += count.total,
            CheckStatus::Failure => down += count.total,
            CheckStatus::Maintenance => {}
        }
    }
    ratio(up, down)
}

fn long_window_today(mine: &[DailyCount], today: i64) -> Option<f64> {
    mine.iter()
        .find(|count| count.day == today)
        .and_then(|count| ratio(count.success + count.degraded, count.failure))
}

fn ratio(up: i64, down: i64) -> Option<f64> {
    let total = up + down;
    if total == 0 {
        None
    } else {
        Some(up as f64 / total as f64)
    }
}

fn average(sum: i64, count: i64) -> Option<i64> {
    if count <= 0 { None } else { Some(sum / count) }
}

fn bar(bucket: haruvo_domain::DayBucket) -> BarResponse {
    BarResponse {
        day: day_label(bucket.day),
        status: bucket.status.as_str().to_owned(),
        uptime_ratio: bucket.uptime_ratio,
        total: bucket.total,
        avg_latency_ms: bucket.avg_latency_ms,
    }
}

fn check_response(check: CheckRow) -> CheckResponse {
    CheckResponse {
        id: check.id,
        status: check.status,
        status_code: check.status_code,
        latency_ms: check.latency_ms,
        error: check.error,
        region: check.region,
        checked_at: check.checked_at,
    }
}

pub fn day_label(day: i64) -> String {
    DateTime::<Utc>::from_timestamp(day * 86_400, 0)
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_owned())
}

#[derive(Serialize)]
pub struct PageResponse {
    pub id: Id,
    pub title: String,
    pub description: String,
    pub slug: String,
    pub published: bool,
    pub homepage_url: Option<String>,
    pub created_at: TimestampMillis,
    pub components: Vec<ComponentResponse>,
}

#[derive(Serialize)]
pub struct ComponentResponse {
    pub id: Id,
    pub monitor_id: Id,
    pub name: String,
    pub description: String,
    pub position: i64,
}

pub fn page_response(page: &Page, components: &[Component]) -> PageResponse {
    PageResponse {
        id: page.id,
        title: page.title.clone(),
        description: page.description.clone(),
        slug: page.slug.clone(),
        published: page.published,
        homepage_url: page.homepage_url.clone(),
        created_at: page.created_at,
        components: components
            .iter()
            .map(|component| ComponentResponse {
                id: component.id,
                monitor_id: component.monitor_id,
                name: component.name.clone(),
                description: component.description.clone(),
                position: component.position,
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct UpdateResponse {
    pub id: Id,
    pub status: IncidentStatus,
    pub message: String,
    pub created_at: TimestampMillis,
}

#[derive(Serialize)]
pub struct IncidentResponse {
    pub id: Id,
    pub page_id: Option<Id>,
    pub monitor_id: Option<Id>,
    pub title: String,
    pub status: IncidentStatus,
    pub impact: IncidentImpact,
    pub auto: bool,
    pub started_at: TimestampMillis,
    pub resolved_at: Option<TimestampMillis>,
    pub updates: Vec<UpdateResponse>,
}

pub fn incident_response(incident: &Incident, updates: &[IncidentUpdate]) -> IncidentResponse {
    IncidentResponse {
        id: incident.id,
        page_id: incident.page_id,
        monitor_id: incident.monitor_id,
        title: incident.title.clone(),
        status: incident.status,
        impact: incident.impact,
        auto: incident.auto,
        started_at: incident.started_at,
        resolved_at: incident.resolved_at,
        updates: updates
            .iter()
            .filter(|update| update.incident_id == incident.id)
            .map(|update| UpdateResponse {
                id: update.id,
                status: update.status,
                message: update.message.clone(),
                created_at: update.created_at,
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct MaintenanceResponse {
    pub id: Id,
    pub page_id: Option<Id>,
    pub title: String,
    pub message: String,
    pub starts_at: TimestampMillis,
    pub ends_at: TimestampMillis,
    pub monitor_ids: Vec<Id>,
    pub active: bool,
}

pub fn maintenance_response(
    maintenance: &Maintenance,
    now: TimestampMillis,
) -> MaintenanceResponse {
    let now = now.as_millis();
    MaintenanceResponse {
        id: maintenance.id,
        page_id: maintenance.page_id,
        title: maintenance.title.clone(),
        message: maintenance.message.clone(),
        starts_at: maintenance.starts_at,
        ends_at: maintenance.ends_at,
        monitor_ids: maintenance.monitor_ids.clone(),
        active: maintenance.starts_at.as_millis() <= now && maintenance.ends_at.as_millis() >= now,
    }
}

#[derive(Serialize)]
pub struct DeliveryResponse {
    pub id: Id,
    pub event: String,
    pub status: String,
    pub error: Option<String>,
    pub created_at: TimestampMillis,
}

#[derive(Serialize)]
pub struct ChannelResponse {
    pub id: Id,
    pub name: String,
    pub provider: String,
    pub config: Value,
    pub enabled: bool,
    pub monitor_ids: Vec<Id>,
    pub deliveries: Vec<DeliveryResponse>,
}

pub fn channel_response(
    channel: &Channel,
    monitor_ids: Vec<Id>,
    deliveries: Vec<Delivery>,
) -> ChannelResponse {
    ChannelResponse {
        id: channel.id,
        name: channel.name.clone(),
        provider: channel.provider.clone(),
        config: serde_json::from_str(&channel.config_json).unwrap_or(Value::Null),
        enabled: channel.enabled,
        monitor_ids,
        deliveries: deliveries
            .into_iter()
            .map(|delivery| DeliveryResponse {
                id: delivery.id,
                event: delivery.event,
                status: delivery.status,
                error: delivery.error,
                created_at: delivery.created_at,
            })
            .collect(),
    }
}

#[derive(Serialize)]
pub struct AuditResponse {
    pub id: Id,
    pub action: String,
    pub resource_type: String,
    pub resource_id: Option<String>,
    pub occurred_at: TimestampMillis,
}

impl From<&Audit> for AuditResponse {
    fn from(audit: &Audit) -> Self {
        Self {
            id: audit.id,
            action: audit.action.clone(),
            resource_type: audit.resource_type.clone(),
            resource_id: audit.resource_id.clone(),
            occurred_at: audit.occurred_at,
        }
    }
}

#[derive(Serialize)]
pub struct PublicComponent {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub state: MonitorState,
    pub uptime_90d: Option<f64>,
    pub bars: Vec<BarResponse>,
}

#[derive(Serialize)]
pub struct PublicPage {
    pub title: String,
    pub description: String,
    pub slug: String,
    pub homepage_url: Option<String>,
    pub banner: &'static str,
    pub components: Vec<PublicComponent>,
    pub incidents: Vec<IncidentResponse>,
    pub maintenances: Vec<MaintenanceResponse>,
}

pub fn banner_label(banner: PageBanner) -> &'static str {
    banner.as_str()
}

/// An agent counts as online when it polled within this window. Agents poll every 5 s.
const AGENT_ONLINE_MS: i64 = 30_000;

#[derive(Serialize)]
pub struct AgentResponse {
    pub id: Id,
    pub name: String,
    pub last_seen_at: Option<TimestampMillis>,
    pub online: bool,
    pub monitor_count: i64,
    pub created_at: TimestampMillis,
}

impl From<&repo::Agent> for AgentResponse {
    fn from(agent: &repo::Agent) -> Self {
        let now = TimestampMillis::now().as_millis();
        Self {
            id: agent.id,
            name: agent.name.clone(),
            last_seen_at: agent.last_seen_at,
            online: agent
                .last_seen_at
                .is_some_and(|seen| now - seen.as_millis() < AGENT_ONLINE_MS),
            monitor_count: agent.monitor_count,
            created_at: agent.created_at,
        }
    }
}
