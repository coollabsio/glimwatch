//! Aggregate for the post-login dashboard.
//!
//! One request and a fixed set of statements. Uptime and latency come from
//! `monitor_daily_stats` for the current unix day. Visitors, pageviews, and
//! clicks come from the insight daily rollups: the last seven midnights
//! against the seven before. A down monitor's "since" is its open automatic
//! incident. A degraded monitor has no incident, so "since" is the first
//! unhealthy check after the last success. Those checks are read only for
//! monitors that need them.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::State;
use axum_extra::extract::cookie::CookieJar;
use chrono::{DateTime, Duration, SecondsFormat, Timelike, Utc};
use owlee_domain::{IncidentImpact, MonitorState, OpenIncident, page_banner};
use owlee_insights::day;
use owlee_platform::{Database, TimestampMillis};
use serde::Serialize;
use sqlx::Row;

use crate::error::AppError;
use crate::routes::require_user;
use crate::state::AppState;

const RANGE_DAYS: i64 = 7;
const DAY_SECS: i64 = 86_400;

#[derive(Serialize)]
pub struct Dashboard {
    pub monitors: Monitors,
    pub incidents: Incidents,
    pub pages: Vec<PageRow>,
    pub errors: Errors,
    pub analytics: Analytics,
    pub quota_hits: Vec<QuotaNotice>,
}

#[derive(Serialize)]
pub struct Monitors {
    pub up: i64,
    pub down: i64,
    pub degraded: i64,
    pub pending: i64,
    pub paused: i64,
    pub uptime_today: Option<f64>,
    pub avg_latency_ms: Option<i64>,
    pub failing: Vec<FailingMonitor>,
}

#[derive(Serialize)]
pub struct FailingMonitor {
    pub id: String,
    pub name: String,
    pub url: String,
    pub state: String,
    pub since: Option<TimestampMillis>,
    pub uptime_today: Option<f64>,
    pub avg_latency_ms: Option<i64>,
}

#[derive(Serialize)]
pub struct Incidents {
    pub open: i64,
    pub items: Vec<IncidentItem>,
}

#[derive(Serialize)]
pub struct IncidentItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub impact: String,
    pub started_at: TimestampMillis,
}

#[derive(Serialize)]
pub struct PageRow {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub published: bool,
    pub banner: &'static str,
}

#[derive(Serialize)]
pub struct Errors {
    pub projects: i64,
    pub unresolved: i64,
    pub new_24h: i64,
    pub events_24h: Vec<i64>,
    pub issues: Vec<IssueItem>,
}

#[derive(Serialize)]
pub struct IssueItem {
    pub id: String,
    pub short_id: Option<String>,
    pub title: String,
    pub project_name: String,
    pub level: String,
    pub event_count: i64,
    pub last_seen: String,
}

#[derive(Serialize)]
pub struct Analytics {
    pub sites: i64,
    pub links: i64,
    pub days: i64,
    pub visitors: i64,
    pub pageviews: i64,
    pub previous_visitors: i64,
    pub previous_pageviews: i64,
    pub clicks: i64,
    pub previous_clicks: i64,
    pub top_sites: Vec<SiteTraffic>,
}

#[derive(Serialize)]
pub struct QuotaNotice {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub used: i64,
    pub quota: i64,
    pub resets_at: String,
}

#[derive(Serialize)]
pub struct SiteTraffic {
    pub id: String,
    pub name: String,
    pub domain: String,
    pub visitors: i64,
    pub pageviews: i64,
}

pub async fn show(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Dashboard>, AppError> {
    require_user(&state, &jar).await?;
    let now = TimestampMillis::now();
    Ok(Json(Dashboard {
        monitors: monitors(&state.db, now).await?,
        incidents: incidents(&state.db).await?,
        pages: pages(&state.db).await?,
        errors: errors(&state.db).await?,
        analytics: analytics(&state.db).await?,
        quota_hits: quota_notices(&state.db).await?,
    }))
}

async fn quota_notices(db: &Database) -> Result<Vec<QuotaNotice>, AppError> {
    let resets_at = crate::quota::resets_at();
    Ok(crate::quota::hits(db)
        .await?
        .into_iter()
        .map(|hit| QuotaNotice {
            kind: hit.kind.to_owned(),
            id: hit.id,
            name: hit.name,
            used: hit.used,
            quota: hit.quota,
            resets_at: resets_at.clone(),
        })
        .collect())
}

async fn monitors(db: &Database, now: TimestampMillis) -> Result<Monitors, AppError> {
    let counts = sqlx::query(
        "SELECT state, CAST(active AS BIGINT) AS active, CAST(COUNT(*) AS BIGINT) AS n \
         FROM monitors GROUP BY state, active",
    )
    .fetch_all(db.pool())
    .await?;
    let mut summary = Monitors {
        up: 0,
        down: 0,
        degraded: 0,
        pending: 0,
        paused: 0,
        uptime_today: None,
        avg_latency_ms: None,
        failing: Vec::new(),
    };
    for row in &counts {
        let state: String = row.try_get("state")?;
        let active: i64 = row.try_get("active")?;
        let n: i64 = row.try_get("n")?;
        if active == 0 {
            summary.paused += n;
            continue;
        }
        match state.as_str() {
            "up" => summary.up += n,
            "down" => summary.down += n,
            "degraded" => summary.degraded += n,
            "pending" => summary.pending += n,
            _ => {}
        }
    }

    let today = now.unix_day();
    let rollup = sqlx::query(
        "SELECT CAST(COALESCE(SUM(success), 0) AS BIGINT) AS success, \
                CAST(COALESCE(SUM(degraded), 0) AS BIGINT) AS degraded, \
                CAST(COALESCE(SUM(failure), 0) AS BIGINT) AS failure, \
                CAST(COALESCE(SUM(latency_sum), 0) AS BIGINT) AS latency_sum, \
                CAST(COALESCE(SUM(latency_count), 0) AS BIGINT) AS latency_count \
         FROM monitor_daily_stats WHERE day = $1",
    )
    .bind(today)
    .fetch_one(db.pool())
    .await?;
    summary.uptime_today = ratio(
        rollup.try_get::<i64, _>("success")? + rollup.try_get::<i64, _>("degraded")?,
        rollup.try_get("failure")?,
    );
    summary.avg_latency_ms = average(
        rollup.try_get("latency_sum")?,
        rollup.try_get("latency_count")?,
    );

    let failing = sqlx::query(
        "WITH failing AS ( \
            SELECT id, name, url, state FROM monitors \
            WHERE active = 1 AND state IN ('down', 'degraded') \
         ), \
         needs_streak AS ( \
            SELECT f.id FROM failing f \
            WHERE f.state != 'down' \
               OR NOT EXISTS ( \
                    SELECT 1 FROM incidents i \
                    WHERE i.monitor_id = f.id AND i.auto = 1 AND i.status != 'resolved' \
               ) \
         ), \
         last_ok AS ( \
            SELECT c.monitor_id, MAX(c.checked_at) AS at \
            FROM check_results c \
            INNER JOIN needs_streak n ON n.id = c.monitor_id \
            WHERE c.status = 'success' \
            GROUP BY c.monitor_id \
         ), \
         streak AS ( \
            SELECT c.monitor_id, MIN(c.checked_at) AS at \
            FROM check_results c \
            INNER JOIN needs_streak n ON n.id = c.monitor_id \
            LEFT JOIN last_ok o ON o.monitor_id = c.monitor_id \
            WHERE c.status IN ('failure', 'degraded') \
              AND c.checked_at > COALESCE(o.at, 0) \
            GROUP BY c.monitor_id \
         ), \
         open_inc AS ( \
            SELECT monitor_id, MAX(started_at) AS at FROM incidents \
            WHERE auto = 1 AND status != 'resolved' AND monitor_id IS NOT NULL \
            GROUP BY monitor_id \
         ), \
         day_stats AS ( \
            SELECT monitor_id, \
                   CAST(SUM(success) AS BIGINT) AS success, \
                   CAST(SUM(degraded) AS BIGINT) AS degraded, \
                   CAST(SUM(failure) AS BIGINT) AS failure, \
                   CAST(SUM(latency_sum) AS BIGINT) AS latency_sum, \
                   CAST(SUM(latency_count) AS BIGINT) AS latency_count \
            FROM monitor_daily_stats WHERE day = $1 \
            GROUP BY monitor_id \
         ) \
         SELECT f.id, f.name, f.url, f.state, \
                CASE WHEN f.state = 'down' THEN COALESCE(i.at, s.at) ELSE s.at END AS since_ms, \
                CAST(COALESCE(t.success, 0) AS BIGINT) AS success, \
                CAST(COALESCE(t.degraded, 0) AS BIGINT) AS degraded, \
                CAST(COALESCE(t.failure, 0) AS BIGINT) AS failure, \
                CAST(COALESCE(t.latency_sum, 0) AS BIGINT) AS latency_sum, \
                CAST(COALESCE(t.latency_count, 0) AS BIGINT) AS latency_count \
         FROM failing f \
         LEFT JOIN open_inc i ON i.monitor_id = f.id \
         LEFT JOIN streak s ON s.monitor_id = f.id \
         LEFT JOIN day_stats t ON t.monitor_id = f.id \
         ORDER BY CASE f.state WHEN 'down' THEN 0 ELSE 1 END, since_ms ASC NULLS LAST \
         LIMIT 8",
    )
    .bind(today)
    .fetch_all(db.pool())
    .await?;
    for row in &failing {
        let since_ms: Option<i64> = row.try_get("since_ms")?;
        summary.failing.push(FailingMonitor {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            url: row.try_get("url")?,
            state: row.try_get("state")?,
            since: since_ms
                .filter(|millis| *millis > 0)
                .map(TimestampMillis::from_millis),
            uptime_today: ratio(
                row.try_get::<i64, _>("success")? + row.try_get::<i64, _>("degraded")?,
                row.try_get("failure")?,
            ),
            avg_latency_ms: average(row.try_get("latency_sum")?, row.try_get("latency_count")?),
        });
    }
    Ok(summary)
}

async fn incidents(db: &Database) -> Result<Incidents, AppError> {
    let open: i64 = sqlx::query(
        "SELECT CAST(COUNT(*) AS BIGINT) AS n FROM incidents WHERE status != 'resolved'",
    )
    .fetch_one(db.pool())
    .await?
    .try_get("n")?;
    let rows = sqlx::query(
        "SELECT id, title, status, impact, started_at FROM incidents \
         WHERE status != 'resolved' ORDER BY started_at DESC LIMIT 8",
    )
    .fetch_all(db.pool())
    .await?;
    let mut items = Vec::new();
    for row in &rows {
        let started_at: i64 = row.try_get("started_at")?;
        items.push(IncidentItem {
            id: row.try_get("id")?,
            title: row.try_get("title")?,
            status: row.try_get("status")?,
            impact: row.try_get("impact")?,
            started_at: TimestampMillis::from_millis(started_at),
        });
    }
    Ok(Incidents { open, items })
}

struct PageAccum {
    id: String,
    title: String,
    slug: String,
    published: bool,
    states: Vec<MonitorState>,
    impacts: Vec<IncidentImpact>,
}

async fn pages(db: &Database) -> Result<Vec<PageRow>, AppError> {
    let rows = sqlx::query(
        "SELECT p.id, p.title, p.slug, CAST(p.published AS BIGINT) AS published, m.state \
         FROM status_pages p \
         LEFT JOIN page_components c ON c.page_id = p.id \
         LEFT JOIN monitors m ON m.id = c.monitor_id \
         ORDER BY lower(p.title), c.position",
    )
    .fetch_all(db.pool())
    .await?;
    let mut pages = Vec::<PageAccum>::new();
    for row in &rows {
        let id: String = row.try_get("id")?;
        let state = row
            .try_get::<Option<String>, _>("state")?
            .as_deref()
            .and_then(MonitorState::parse);
        if let Some(page) = pages.iter_mut().find(|page| page.id == id) {
            if let Some(state) = state {
                page.states.push(state);
            }
            continue;
        }
        pages.push(PageAccum {
            id,
            title: row.try_get("title")?,
            slug: row.try_get("slug")?,
            published: row.try_get::<i64, _>("published")? != 0,
            states: state.into_iter().collect(),
            impacts: Vec::new(),
        });
    }

    let impacts = sqlx::query(
        "SELECT page_id, impact FROM incidents \
         WHERE status != 'resolved' AND page_id IS NOT NULL",
    )
    .fetch_all(db.pool())
    .await?;
    for row in &impacts {
        let page_id: String = row.try_get("page_id")?;
        let Some(impact) = row
            .try_get::<String, _>("impact")
            .ok()
            .as_deref()
            .and_then(IncidentImpact::parse)
        else {
            continue;
        };
        if let Some(page) = pages.iter_mut().find(|page| page.id == page_id) {
            page.impacts.push(impact);
        }
    }

    Ok(pages
        .into_iter()
        .map(|page| {
            let open = page
                .impacts
                .iter()
                .copied()
                .map(|impact| OpenIncident { impact })
                .collect::<Vec<_>>();
            PageRow {
                id: page.id,
                title: page.title,
                slug: page.slug,
                published: page.published,
                banner: page_banner(&page.states, &open).as_str(),
            }
        })
        .collect())
}

async fn errors(db: &Database) -> Result<Errors, AppError> {
    let start = trend_start(Utc::now());
    let cutoff = start.to_rfc3339_opts(SecondsFormat::Millis, true);
    let counts = sqlx::query(
        "SELECT \
            (SELECT CAST(COUNT(*) AS BIGINT) FROM projects) AS projects, \
            (SELECT CAST(COUNT(*) AS BIGINT) FROM issues WHERE status = 'unresolved') AS unresolved, \
            (SELECT CAST(COUNT(*) AS BIGINT) FROM issues \
             WHERE status = 'unresolved' AND first_seen >= $1) AS new_24h",
    )
    .bind(&cutoff)
    .fetch_one(db.pool())
    .await?;

    let issue_rows = sqlx::query(
        "SELECT i.id, i.short_id, i.title, i.level, i.event_count, i.last_seen, \
                p.name AS project_name \
         FROM issues i \
         JOIN projects p ON p.id = i.project_id \
         WHERE i.status = 'unresolved' \
         ORDER BY i.last_seen DESC \
         LIMIT 5",
    )
    .fetch_all(db.pool())
    .await?;
    let mut issues = Vec::new();
    for row in &issue_rows {
        issues.push(IssueItem {
            id: row.try_get("id")?,
            short_id: row.try_get("short_id")?,
            title: row.try_get("title")?,
            project_name: row.try_get("project_name")?,
            level: row.try_get("level")?,
            event_count: row.try_get("event_count")?,
            last_seen: row.try_get("last_seen")?,
        });
    }

    let buckets = sqlx::query(
        "SELECT substr(timestamp, 1, 13) AS bucket, CAST(COUNT(*) AS BIGINT) AS n \
         FROM events WHERE timestamp >= $1 \
         GROUP BY substr(timestamp, 1, 13)",
    )
    .bind(&cutoff)
    .fetch_all(db.pool())
    .await?;
    let mut by_hour = BTreeMap::new();
    for row in &buckets {
        by_hour.insert(
            row.try_get::<String, _>("bucket")?,
            row.try_get::<i64, _>("n")?,
        );
    }
    let mut events_24h = Vec::with_capacity(24);
    for index in 0..24 {
        let hour = (start + Duration::hours(index))
            .format("%Y-%m-%dT%H")
            .to_string();
        events_24h.push(by_hour.get(&hour).copied().unwrap_or(0));
    }

    Ok(Errors {
        projects: counts.try_get("projects")?,
        unresolved: counts.try_get("unresolved")?,
        new_24h: counts.try_get("new_24h")?,
        events_24h,
        issues,
    })
}

async fn analytics(db: &Database) -> Result<Analytics, AppError> {
    let today = day(owlee_insights::now_secs());
    let current_from = today - (RANGE_DAYS - 1) * DAY_SECS;
    let previous_from = today - (RANGE_DAYS * 2 - 1) * DAY_SECS;
    let counts = sqlx::query(
        "SELECT \
            (SELECT CAST(COUNT(*) AS BIGINT) FROM insight_sites) AS sites, \
            (SELECT CAST(COUNT(*) AS BIGINT) FROM insight_links) AS links",
    )
    .fetch_one(db.pool())
    .await?;
    let traffic = sqlx::query(
        "SELECT \
            CAST(COALESCE(SUM(CASE WHEN day >= $1 THEN pageviews ELSE 0 END), 0) AS BIGINT) AS pageviews, \
            CAST(COALESCE(SUM(CASE WHEN day >= $1 THEN visitors ELSE 0 END), 0) AS BIGINT) AS visitors, \
            CAST(COALESCE(SUM(CASE WHEN day < $1 THEN pageviews ELSE 0 END), 0) AS BIGINT) AS previous_pageviews, \
            CAST(COALESCE(SUM(CASE WHEN day < $1 THEN visitors ELSE 0 END), 0) AS BIGINT) AS previous_visitors \
         FROM insight_daily WHERE day >= $2 AND day <= $3",
    )
    .bind(current_from)
    .bind(previous_from)
    .bind(today)
    .fetch_one(db.pool())
    .await?;
    let clicks = sqlx::query(
        "SELECT \
            CAST(COALESCE(SUM(CASE WHEN day >= $1 THEN clicks ELSE 0 END), 0) AS BIGINT) AS clicks, \
            CAST(COALESCE(SUM(CASE WHEN day < $1 THEN clicks ELSE 0 END), 0) AS BIGINT) AS previous_clicks \
         FROM insight_link_daily WHERE day >= $2 AND day <= $3",
    )
    .bind(current_from)
    .bind(previous_from)
    .bind(today)
    .fetch_one(db.pool())
    .await?;
    let sites = sqlx::query(
        "SELECT s.id, s.name, s.domain, \
                CAST(COALESCE(SUM(d.pageviews), 0) AS BIGINT) AS pageviews, \
                CAST(COALESCE(SUM(d.visitors), 0) AS BIGINT) AS visitors \
         FROM insight_sites s \
         LEFT JOIN insight_daily d ON d.site_id = s.id AND d.day >= $1 AND d.day <= $2 \
         GROUP BY s.id, s.name, s.domain \
         ORDER BY pageviews DESC, s.name \
         LIMIT 5",
    )
    .bind(current_from)
    .bind(today)
    .fetch_all(db.pool())
    .await?;
    let mut top_sites = Vec::new();
    for row in &sites {
        top_sites.push(SiteTraffic {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            domain: row.try_get("domain")?,
            visitors: row.try_get("visitors")?,
            pageviews: row.try_get("pageviews")?,
        });
    }
    Ok(Analytics {
        sites: counts.try_get("sites")?,
        links: counts.try_get("links")?,
        days: RANGE_DAYS,
        visitors: traffic.try_get("visitors")?,
        pageviews: traffic.try_get("pageviews")?,
        previous_visitors: traffic.try_get("previous_visitors")?,
        previous_pageviews: traffic.try_get("previous_pageviews")?,
        clicks: clicks.try_get("clicks")?,
        previous_clicks: clicks.try_get("previous_clicks")?,
        top_sites,
    })
}

fn trend_start(now: DateTime<Utc>) -> DateTime<Utc> {
    let shifted = now - Duration::hours(23);
    shifted
        .with_minute(0)
        .and_then(|value| value.with_second(0))
        .and_then(|value| value.with_nanosecond(0))
        .unwrap_or(shifted)
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
