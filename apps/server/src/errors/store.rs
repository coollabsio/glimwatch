use chrono::{DateTime, Duration, SecondsFormat, Utc};
use haruvo_errors::NormalizedEvent;
use haruvo_platform::{Backend, Database};
use rand::RngCore;
use serde::Serialize;
use sqlx::Row;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub public_key: String,
    pub platform: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRow {
    pub id: String,
    pub project_id: String,
    pub project_name: String,
    pub project_slug: String,
    pub title: String,
    pub culprit: Option<String>,
    pub status: String,
    pub level: String,
    pub platform: Option<String>,
    pub environment: Option<String>,
    pub release: Option<String>,
    pub first_seen: String,
    pub last_seen: String,
    pub event_count: i64,
    pub short_id: String,
    pub unhandled: bool,
    pub priority: String,
    pub user_count: i64,
    pub period_event_count: i64,
    pub trend24h: Vec<i64>,
    pub trend14d: Vec<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSummary {
    pub id: String,
    pub timestamp: String,
    pub level: String,
    pub title: String,
    pub environment: Option<String>,
    pub release: Option<String>,
}

pub struct StoredEvent {
    pub summary: EventSummary,
    pub payload: String,
}

fn statement(db: &Database, sql: &str) -> String {
    // sqlx Any uses `$1` on both SQLite and Postgres.
    let _ = db.backend();
    let mut rewritten = String::with_capacity(sql.len());
    let mut index = 1;
    let mut quoted = false;
    for character in sql.chars() {
        if character == '\'' {
            quoted = !quoted;
            rewritten.push(character);
        } else if character == '?' && !quoted {
            rewritten.push('$');
            rewritten.push_str(&index.to_string());
            index += 1;
        } else {
            rewritten.push(character);
        }
    }
    let _ = Backend::Sqlite;
    rewritten
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub async fn list_projects(db: &Database) -> Result<Vec<Project>, AppError> {
    let sql = statement(
        db,
        "SELECT id, name, slug, public_key, platform, created_at FROM projects ORDER BY created_at ASC",
    );
    let rows = sqlx::query(&sql).fetch_all(db.pool()).await?;
    rows.iter().map(project_from_row).collect()
}

pub async fn get_project(db: &Database, id: &str) -> Result<Project, AppError> {
    let sql = statement(
        db,
        "SELECT id, name, slug, public_key, platform, created_at FROM projects WHERE id = ?",
    );
    let row = sqlx::query(&sql).bind(id).fetch_optional(db.pool()).await?;
    match row {
        Some(row) => project_from_row(&row),
        None => Err(AppError::NotFound),
    }
}

pub async fn project_for_ingest(
    db: &Database,
    id: &str,
    public_key: &str,
) -> Result<Project, AppError> {
    let project = get_project(db, id).await?;
    if project.public_key != public_key {
        return Err(AppError::Unauthorized);
    }
    Ok(project)
}

pub async fn create_project(
    db: &Database,
    name: &str,
    platform: &str,
) -> Result<Project, AppError> {
    let name = name.trim();
    if name.is_empty() || name.len() > 120 {
        return Err(AppError::BadRequest(
            "Project name must be between 1 and 120 characters".into(),
        ));
    }
    let platform = normalize_platform(platform);
    let slug = unique_slug(db, &slugify(name)).await?;
    let mut key_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut key_bytes);
    let public_key = hex::encode(key_bytes);
    let created_at = now();
    let mut tx = db.pool().begin().await?;
    let sql = statement(
        db,
        "UPDATE project_counters SET next_id = next_id + 1 WHERE id = 1 RETURNING next_id",
    );
    let row = sqlx::query(&sql).fetch_one(&mut *tx).await?;
    let next_id: i64 = row.try_get("next_id")?;
    let id = (next_id - 1).to_string();
    let sql = statement(
        db,
        "INSERT INTO projects (id, name, slug, public_key, platform, created_at) VALUES (?, ?, ?, ?, ?, ?)",
    );
    sqlx::query(&sql)
        .bind(&id)
        .bind(name)
        .bind(&slug)
        .bind(&public_key)
        .bind(&platform)
        .bind(&created_at)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(Project {
        id,
        name: name.to_owned(),
        slug,
        public_key,
        platform,
        created_at,
    })
}

pub async fn delete_project(db: &Database, id: &str) -> Result<(), AppError> {
    let mut tx = db.pool().begin().await?;
    let sql = statement(db, "DELETE FROM events WHERE project_id = ?");
    sqlx::query(&sql).bind(id).execute(&mut *tx).await?;
    let sql = statement(db, "DELETE FROM issues WHERE project_id = ?");
    sqlx::query(&sql).bind(id).execute(&mut *tx).await?;
    let sql = statement(db, "DELETE FROM projects WHERE id = ?");
    let result = sqlx::query(&sql).bind(id).execute(&mut *tx).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    tx.commit().await?;
    Ok(())
}

#[derive(Default)]
pub struct IssueQuery {
    pub status: String,
    pub project_id: String,
    pub search: String,
    pub environment: String,
    pub since: String,
    pub sort: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub key: String,
    pub value: String,
    pub count: i64,
}

pub struct IssueAnalytics {
    pub histogram: Vec<i64>,
    pub users: i64,
    pub period_events: i64,
    pub tags: Vec<TagCount>,
}

pub async fn list_issues(db: &Database, query: &IssueQuery) -> Result<Vec<IssueRow>, AppError> {
    let like = like_pattern(&query.search);
    let search_flag = if query.search.trim().is_empty() {
        String::new()
    } else {
        "1".to_owned()
    };
    let since = since_bound(&query.since);
    let since_flag = if since.is_empty() {
        String::new()
    } else {
        "1".into()
    };
    let sql = statement(
        db,
        "SELECT i.id, i.project_id, p.name AS project_name, p.slug AS project_slug,
                i.title, i.culprit, i.status, i.level, i.platform, i.environment, i.release,
                i.first_seen, i.last_seen, i.event_count, i.short_id, i.unhandled
         FROM issues i
         JOIN projects p ON p.id = i.project_id
         WHERE (? = '' OR i.status = ?)
           AND (? = '' OR i.project_id = ?)
           AND (? = '' OR lower(i.title) LIKE ? ESCAPE '\\' OR lower(COALESCE(i.culprit, '')) LIKE ? ESCAPE '\\' OR lower(COALESCE(i.short_id, '')) LIKE ? ESCAPE '\\')
           AND (? = '' OR EXISTS (SELECT 1 FROM events ev WHERE ev.issue_id = i.id AND ev.environment = ?))
           AND (? = '' OR EXISTS (SELECT 1 FROM events ev WHERE ev.issue_id = i.id AND ev.timestamp >= ?))
         ORDER BY i.last_seen DESC
         LIMIT 200",
    );
    let rows = sqlx::query(&sql)
        .bind(&query.status)
        .bind(&query.status)
        .bind(&query.project_id)
        .bind(&query.project_id)
        .bind(&search_flag)
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(&query.environment)
        .bind(&query.environment)
        .bind(&since_flag)
        .bind(&since)
        .fetch_all(db.pool())
        .await?;
    let mut issues = rows
        .iter()
        .map(issue_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    attach_feed_stats(db, &mut issues, &since).await?;
    sort_issues(&mut issues, &query.sort);
    Ok(issues)
}

pub async fn list_environments(db: &Database, project_id: &str) -> Result<Vec<String>, AppError> {
    let sql = statement(
        db,
        "SELECT DISTINCT environment FROM events
         WHERE environment IS NOT NULL AND environment != ''
           AND (? = '' OR project_id = ?)
         ORDER BY environment",
    );
    let rows = sqlx::query(&sql)
        .bind(project_id)
        .bind(project_id)
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .iter()
        .filter_map(|row| row.try_get::<String, _>("environment").ok())
        .collect())
}

pub async fn get_issue(db: &Database, id: &str) -> Result<IssueRow, AppError> {
    let sql = statement(
        db,
        "SELECT i.id, i.project_id, p.name AS project_name, p.slug AS project_slug,
                i.title, i.culprit, i.status, i.level, i.platform, i.environment, i.release,
                i.first_seen, i.last_seen, i.event_count, i.short_id, i.unhandled
         FROM issues i
         JOIN projects p ON p.id = i.project_id
         WHERE i.id = ? OR i.short_id = ?",
    );
    let row = sqlx::query(&sql)
        .bind(id)
        .bind(id)
        .fetch_optional(db.pool())
        .await?;
    match row {
        Some(row) => issue_from_row(&row),
        None => Err(AppError::NotFound),
    }
}

pub async fn update_issue_status(
    db: &Database,
    id: &str,
    status: &str,
) -> Result<IssueRow, AppError> {
    if !matches!(status, "unresolved" | "resolved" | "ignored") {
        return Err(AppError::BadRequest(
            "Status must be unresolved, resolved, or ignored".into(),
        ));
    }
    let sql = statement(db, "UPDATE issues SET status = ? WHERE id = ?");
    let result = sqlx::query(&sql)
        .bind(status)
        .bind(id)
        .execute(db.pool())
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    get_issue(db, id).await
}

pub async fn list_events(db: &Database, issue_id: &str) -> Result<Vec<EventSummary>, AppError> {
    let sql = statement(
        db,
        "SELECT id, timestamp, level, title, environment, release
         FROM events WHERE issue_id = ? ORDER BY timestamp DESC LIMIT 50",
    );
    let rows = sqlx::query(&sql)
        .bind(issue_id)
        .fetch_all(db.pool())
        .await?;
    Ok(rows.iter().map(summary_from_row).collect())
}

pub async fn get_event(db: &Database, id: &str) -> Result<StoredEvent, AppError> {
    let sql = statement(
        db,
        "SELECT id, timestamp, level, title, environment, release, payload FROM events WHERE id = ?",
    );
    let row = sqlx::query(&sql).bind(id).fetch_optional(db.pool()).await?;
    let Some(row) = row else {
        return Err(AppError::NotFound);
    };
    Ok(StoredEvent {
        summary: summary_from_row(&row),
        payload: row.try_get("payload")?,
    })
}

pub async fn latest_event(db: &Database, issue_id: &str) -> Result<Option<StoredEvent>, AppError> {
    let sql = statement(
        db,
        "SELECT id, timestamp, level, title, environment, release, payload
         FROM events WHERE issue_id = ? ORDER BY timestamp DESC LIMIT 1",
    );
    let row = sqlx::query(&sql)
        .bind(issue_id)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.map(|row| StoredEvent {
        summary: summary_from_row(&row),
        payload: row.try_get("payload").unwrap_or_else(|_| "{}".to_owned()),
    }))
}

pub async fn ingest_event(
    db: &Database,
    project: &Project,
    event: &NormalizedEvent,
) -> Result<String, AppError> {
    let mut tx = db.pool().begin().await?;
    let sql = statement(db, "SELECT id FROM events WHERE id = ?");
    let existing = sqlx::query(&sql)
        .bind(&event.event_id)
        .fetch_optional(&mut *tx)
        .await?;
    if existing.is_some() {
        tx.commit().await?;
        return Ok(event.event_id.clone());
    }
    let issue_id = Uuid::new_v4().to_string();
    let seq_sql = statement(
        db,
        "UPDATE projects SET next_issue = next_issue + 1 WHERE id = ? RETURNING next_issue, slug",
    );
    let seq = sqlx::query(&seq_sql)
        .bind(&project.id)
        .fetch_one(&mut *tx)
        .await?;
    let next_issue: i64 = seq.try_get("next_issue")?;
    let slug: String = seq.try_get("slug")?;
    let short_id = format!("{}-{}", short_prefix(&slug), next_issue - 1);
    let unhandled = if event_unhandled(event) { 1 } else { 0 };
    let sql = statement(
        db,
        "INSERT INTO issues (
            id, project_id, fingerprint, title, culprit, status, level, platform,
            environment, release, first_seen, last_seen, event_count, short_id, unhandled
         ) VALUES (?, ?, ?, ?, ?, 'unresolved', ?, ?, ?, ?, ?, ?, 1, ?, ?)
         ON CONFLICT (project_id, fingerprint) DO UPDATE SET
            title = excluded.title,
            culprit = excluded.culprit,
            level = excluded.level,
            platform = excluded.platform,
            environment = excluded.environment,
            release = excluded.release,
            last_seen = excluded.last_seen,
            event_count = issues.event_count + 1,
            unhandled = CASE WHEN excluded.unhandled = 1 THEN 1 ELSE issues.unhandled END,
            status = CASE WHEN issues.status = 'resolved' THEN 'unresolved' ELSE issues.status END
         RETURNING id",
    );
    let row = sqlx::query(&sql)
        .bind(&issue_id)
        .bind(&project.id)
        .bind(&event.fingerprint)
        .bind(&event.title)
        .bind(&event.culprit)
        .bind(&event.level)
        .bind(event.platform.as_deref().unwrap_or(&project.platform))
        .bind(&event.environment)
        .bind(&event.release)
        .bind(&event.timestamp)
        .bind(&event.timestamp)
        .bind(&short_id)
        .bind(unhandled)
        .fetch_one(&mut *tx)
        .await?;
    let issue_id: String = row.try_get("id")?;
    let payload = serde_json::to_string(&event.raw).unwrap_or_else(|_| "{}".to_owned());
    let insert_event = statement(
        db,
        "INSERT INTO events (
            id, project_id, issue_id, timestamp, level, title, environment, release, user_key, payload
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    );
    sqlx::query(&insert_event)
        .bind(&event.event_id)
        .bind(&project.id)
        .bind(&issue_id)
        .bind(&event.timestamp)
        .bind(&event.level)
        .bind(&event.title)
        .bind(&event.environment)
        .bind(&event.release)
        .bind(user_key_of(event).unwrap_or_default())
        .bind(payload)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(event.event_id.clone())
}

fn project_from_row(row: &sqlx::any::AnyRow) -> Result<Project, AppError> {
    Ok(Project {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        slug: row.try_get("slug")?,
        public_key: row.try_get("public_key")?,
        platform: row.try_get("platform")?,
        created_at: row.try_get("created_at")?,
    })
}

fn issue_from_row(row: &sqlx::any::AnyRow) -> Result<IssueRow, AppError> {
    Ok(IssueRow {
        id: row.try_get("id")?,
        project_id: row.try_get("project_id")?,
        project_name: row.try_get("project_name")?,
        project_slug: row.try_get("project_slug")?,
        title: row.try_get("title")?,
        culprit: row.try_get("culprit")?,
        status: row.try_get("status")?,
        level: row.try_get("level")?,
        platform: row.try_get("platform")?,
        environment: row.try_get("environment")?,
        release: row.try_get("release")?,
        first_seen: row.try_get("first_seen")?,
        last_seen: row.try_get("last_seen")?,
        event_count: row.try_get("event_count")?,
        short_id: row
            .try_get::<Option<String>, _>("short_id")?
            .unwrap_or_default(),
        unhandled: row.try_get::<i64, _>("unhandled").unwrap_or(0) != 0,
        priority: priority_for(&row.try_get::<String, _>("level")?),
        user_count: 0,
        period_event_count: row.try_get("event_count")?,
        trend24h: vec![0; 24],
        trend14d: vec![0; 14],
    })
}

fn summary_from_row(row: &sqlx::any::AnyRow) -> EventSummary {
    EventSummary {
        id: row.get("id"),
        timestamp: row.get("timestamp"),
        level: row.get("level"),
        title: row.get("title"),
        environment: row.get("environment"),
        release: row.get("release"),
    }
}

fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if (ch == ' ' || ch == '-' || ch == '_') && !slug.ends_with('-') && !slug.is_empty()
        {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-').to_owned();
    if slug.is_empty() {
        "project".to_owned()
    } else {
        slug
    }
}

async fn unique_slug(db: &Database, base: &str) -> Result<String, AppError> {
    let mut candidate = base.to_owned();
    let mut suffix = 2;
    loop {
        let sql = statement(db, "SELECT id FROM projects WHERE slug = ?");
        let existing = sqlx::query(&sql)
            .bind(&candidate)
            .fetch_optional(db.pool())
            .await?;
        if existing.is_none() {
            return Ok(candidate);
        }
        candidate = format!("{base}-{suffix}");
        suffix += 1;
    }
}

fn normalize_platform(platform: &str) -> String {
    let normalized = match platform.trim().to_ascii_lowercase().as_str() {
        "php" => "php".to_owned(),
        "javascript" | "node" | "browser" => "javascript".to_owned(),
        "rust" => "rust".to_owned(),
        "python" => "python".to_owned(),
        "java" => "java".to_owned(),
        "go" | "golang" => "go".to_owned(),
        "ruby" => "ruby".to_owned(),
        "dotnet" | "csharp" => "dotnet".to_owned(),
        "" | "other" => "other".to_owned(),
        other => other
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '-')
            .take(32)
            .collect(),
    };
    if normalized.is_empty() {
        "other".to_owned()
    } else {
        normalized
    }
}

fn like_pattern(search: &str) -> String {
    let mut escaped = String::new();
    for ch in search.trim().chars() {
        if ch == '\\' || ch == '%' || ch == '_' {
            escaped.push('\\');
        }
        escaped.push(ch.to_ascii_lowercase());
    }
    format!("%{escaped}%")
}

pub fn build_dsn(origin: &str, public_key: &str, project_id: &str) -> String {
    let origin = origin.trim().trim_end_matches('/');
    let (scheme, rest) = origin.split_once("://").unwrap_or(("http", origin));
    format!("{scheme}://{public_key}@{rest}/{project_id}")
}

pub async fn update_many_status(
    db: &Database,
    ids: &[String],
    status: &str,
) -> Result<usize, AppError> {
    if !matches!(status, "unresolved" | "resolved" | "ignored") {
        return Err(AppError::BadRequest(
            "Status must be unresolved, resolved, or ignored".into(),
        ));
    }
    let mut updated = 0;
    for id in ids {
        let sql = statement(
            db,
            "UPDATE issues SET status = ? WHERE id = ? OR short_id = ?",
        );
        let result = sqlx::query(&sql)
            .bind(status)
            .bind(id)
            .bind(id)
            .execute(db.pool())
            .await?;
        updated += result.rows_affected() as usize;
    }
    Ok(updated)
}

pub async fn issue_analytics(
    db: &Database,
    issue_id: &str,
    range: &str,
    environment: &str,
    search: &str,
) -> Result<(Vec<EventSummary>, Option<String>, IssueAnalytics), AppError> {
    let sql = statement(
        db,
        "SELECT id, timestamp, level, title, environment, release, user_key, payload
         FROM events WHERE issue_id = ? ORDER BY timestamp DESC LIMIT 1000",
    );
    let rows = sqlx::query(&sql)
        .bind(issue_id)
        .fetch_all(db.pool())
        .await?;
    let start = since_time(range);
    let needle = search.trim().to_ascii_lowercase();
    let mut matched = Vec::new();
    for row in &rows {
        let environment_value: Option<String> = row.try_get("environment")?;
        let title: String = row.try_get("title")?;
        let timestamp: String = row.try_get("timestamp")?;
        if !environment.is_empty() && environment_value.as_deref() != Some(environment) {
            continue;
        }
        if !needle.is_empty() && !title.to_ascii_lowercase().contains(&needle) {
            continue;
        }
        if let Some(start) = start {
            if let Some(parsed) = parse_time(&timestamp) {
                if parsed < start {
                    continue;
                }
            }
        }
        matched.push(row);
    }
    let bucket_count = bucket_count(range);
    let histogram_start = start.unwrap_or_else(|| Utc::now() - Duration::days(14));
    let mut histogram = vec![0i64; bucket_count];
    let mut users = std::collections::BTreeSet::new();
    let mut tags: std::collections::BTreeMap<(String, String), i64> =
        std::collections::BTreeMap::new();
    let mut summaries = Vec::new();
    let mut latest_payload = None;
    for row in &matched {
        let timestamp: String = row.try_get("timestamp")?;
        let user_key: Option<String> = row.try_get("user_key")?;
        let payload: String = row.try_get("payload")?;
        if let Some(user_key) = user_key.filter(|value| !value.is_empty()) {
            users.insert(user_key);
        }
        if let Some(parsed) = parse_time(&timestamp) {
            if let Some(index) = bucket_index(parsed, histogram_start, bucket_count) {
                histogram[index] += 1;
            }
        }
        if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&payload) {
            let normalized = haruvo_errors::normalize_event(raw);
            for tag in normalized.tags {
                *tags.entry((tag.key, tag.value)).or_default() += 1;
            }
        }
        if latest_payload.is_none() {
            latest_payload = Some(payload);
        }
        summaries.push(EventSummary {
            id: row.try_get("id")?,
            timestamp,
            level: row.try_get("level")?,
            title: row.try_get("title")?,
            environment: row.try_get("environment")?,
            release: row.try_get("release")?,
        });
    }
    let mut tag_counts: Vec<TagCount> = tags
        .into_iter()
        .map(|((key, value), count)| TagCount { key, value, count })
        .collect();
    tag_counts.sort_by(|left, right| right.count.cmp(&left.count));
    tag_counts.truncate(8);
    let period_events = matched.len() as i64;
    Ok((
        summaries,
        latest_payload,
        IssueAnalytics {
            histogram,
            users: users.len() as i64,
            period_events,
            tags: tag_counts,
        },
    ))
}

pub async fn backfill_derived(db: &Database) -> Result<(), AppError> {
    let missing = sqlx::query(
        "SELECT i.id, i.project_id, p.slug FROM issues i JOIN projects p ON p.id = i.project_id WHERE i.short_id IS NULL ORDER BY i.first_seen ASC",
    )
    .fetch_all(db.pool())
    .await?;
    for row in &missing {
        let id: String = row.try_get("id")?;
        let project_id: String = row.try_get("project_id")?;
        let slug: String = row.try_get("slug")?;
        let seq_sql = statement(
            db,
            "UPDATE projects SET next_issue = next_issue + 1 WHERE id = ? RETURNING next_issue",
        );
        let seq = sqlx::query(&seq_sql)
            .bind(&project_id)
            .fetch_one(db.pool())
            .await?;
        let next: i64 = seq.try_get("next_issue")?;
        let short_id = format!("{}-{}", short_prefix(&slug), next - 1);
        let update = statement(db, "UPDATE issues SET short_id = ? WHERE id = ?");
        sqlx::query(&update)
            .bind(short_id)
            .bind(id)
            .execute(db.pool())
            .await?;
    }
    let keys = sqlx::query("SELECT id, payload FROM events WHERE user_key IS NULL")
        .fetch_all(db.pool())
        .await?;
    for row in &keys {
        let id: String = row.try_get("id")?;
        let payload: String = row.try_get("payload")?;
        let key = serde_json::from_str::<serde_json::Value>(&payload)
            .ok()
            .map(|raw| user_key_of(&haruvo_errors::normalize_event(raw)))
            .unwrap_or(None);
        let update = statement(db, "UPDATE events SET user_key = ? WHERE id = ?");
        sqlx::query(&update)
            .bind(key.unwrap_or_default())
            .bind(id)
            .execute(db.pool())
            .await?;
    }
    regroup_issues(db).await?;
    Ok(())
}

/// Recompute fingerprints from stored payloads and merge issues that now collide.
/// SDK-internal frames used to split one Rust error into several issues.
pub async fn regroup_issues(db: &Database) -> Result<(), AppError> {
    let rows = sqlx::query(
        "SELECT id, project_id, issue_id, timestamp, level, title, environment, release, payload FROM events",
    )
    .fetch_all(db.pool())
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    struct EventGroup {
        id: String,
        project_id: String,
        issue_id: String,
        timestamp: String,
        fingerprint: String,
        title: String,
        culprit: Option<String>,
        level: String,
        environment: Option<String>,
        release: Option<String>,
        unhandled: bool,
    }
    let mut events = Vec::new();
    for row in &rows {
        let payload: String = row.try_get("payload")?;
        let raw = serde_json::from_str(&payload).unwrap_or(serde_json::Value::Null);
        let normalized = haruvo_errors::normalize_event(raw);
        let unhandled = event_unhandled(&normalized);
        events.push(EventGroup {
            id: row.try_get("id")?,
            project_id: row.try_get("project_id")?,
            issue_id: row.try_get("issue_id")?,
            timestamp: row.try_get("timestamp")?,
            fingerprint: normalized.fingerprint,
            title: normalized.title,
            culprit: normalized.culprit,
            level: normalized.level,
            environment: normalized.environment,
            release: normalized.release,
            unhandled,
        });
    }
    let issue_rows = sqlx::query("SELECT id, fingerprint, first_seen FROM issues")
        .fetch_all(db.pool())
        .await?;
    let mut issue_meta = std::collections::BTreeMap::new();
    for row in &issue_rows {
        let id: String = row.try_get("id")?;
        issue_meta.insert(
            id,
            (
                row.try_get::<String, _>("fingerprint")?,
                row.try_get::<String, _>("first_seen")?,
            ),
        );
    }
    let mut groups: std::collections::BTreeMap<(String, String), Vec<usize>> =
        std::collections::BTreeMap::new();
    for (index, event) in events.iter().enumerate() {
        groups
            .entry((event.project_id.clone(), event.fingerprint.clone()))
            .or_default()
            .push(index);
    }
    let mut tx = db.pool().begin().await?;
    let mut survivors = Vec::new();
    for ((_project_id, fingerprint), indexes) in groups {
        let mut candidates: Vec<String> = indexes
            .iter()
            .map(|index| events[*index].issue_id.clone())
            .collect();
        candidates.sort();
        candidates.dedup();
        let survivor = candidates
            .iter()
            .filter(|id| {
                issue_meta
                    .get(*id)
                    .map(|(current, _)| current == &fingerprint)
                    .unwrap_or(false)
            })
            .min_by_key(|id| {
                issue_meta
                    .get(*id)
                    .map(|(_, seen)| seen.clone())
                    .unwrap_or_default()
            })
            .cloned()
            .or_else(|| {
                candidates.into_iter().min_by_key(|id| {
                    issue_meta
                        .get(id)
                        .map(|(_, seen)| seen.clone())
                        .unwrap_or_default()
                })
            });
        let Some(survivor) = survivor else {
            continue;
        };
        for index in &indexes {
            let event = &events[*index];
            if event.issue_id == survivor {
                continue;
            }
            let sql = statement(db, "UPDATE events SET issue_id = ? WHERE id = ?");
            sqlx::query(&sql)
                .bind(&survivor)
                .bind(&event.id)
                .execute(&mut *tx)
                .await?;
        }
        let mut members: Vec<&EventGroup> = indexes.iter().map(|index| &events[*index]).collect();
        members.sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
        let first = members.first().unwrap();
        let last = members.last().unwrap();
        let unhandled = if members.iter().any(|event| event.unhandled) {
            1
        } else {
            0
        };
        survivors.push((
            survivor,
            fingerprint,
            last.title.clone(),
            last.culprit.clone(),
            last.level.clone(),
            last.environment.clone(),
            last.release.clone(),
            first.timestamp.clone(),
            last.timestamp.clone(),
            members.len() as i64,
            unhandled,
        ));
    }
    sqlx::query("DELETE FROM issues WHERE id NOT IN (SELECT DISTINCT issue_id FROM events)")
        .execute(&mut *tx)
        .await?;
    for (
        id,
        fingerprint,
        title,
        culprit,
        level,
        environment,
        release,
        first_seen,
        last_seen,
        event_count,
        unhandled,
    ) in survivors
    {
        let sql = statement(
            db,
            "UPDATE issues SET fingerprint = ?, title = ?, culprit = ?, level = ?, environment = ?, release = ?, first_seen = ?, last_seen = ?, event_count = ?, unhandled = ? WHERE id = ?",
        );
        sqlx::query(&sql)
            .bind(fingerprint)
            .bind(title)
            .bind(culprit)
            .bind(level)
            .bind(environment)
            .bind(release)
            .bind(first_seen)
            .bind(last_seen)
            .bind(event_count)
            .bind(unhandled)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn attach_feed_stats(
    db: &Database,
    issues: &mut [IssueRow],
    since: &str,
) -> Result<(), AppError> {
    if issues.is_empty() {
        return Ok(());
    }
    let window_start = Utc::now() - Duration::days(14);
    let sql = statement(
        db,
        "SELECT issue_id, timestamp, user_key FROM events WHERE timestamp >= ?",
    );
    let rows = sqlx::query(&sql)
        .bind(window_start.to_rfc3339_opts(SecondsFormat::Millis, true))
        .fetch_all(db.pool())
        .await?;
    let period_start = if since.is_empty() {
        None
    } else {
        parse_time(since)
    };
    let day = Duration::hours(24);
    let mut by_issue: std::collections::BTreeMap<
        String,
        (Vec<i64>, Vec<i64>, i64, std::collections::BTreeSet<String>),
    > = std::collections::BTreeMap::new();
    for issue in issues.iter() {
        by_issue.insert(
            issue.id.clone(),
            (
                vec![0; 24],
                vec![0; 14],
                0,
                std::collections::BTreeSet::new(),
            ),
        );
    }
    for row in &rows {
        let issue_id: String = row.try_get("issue_id")?;
        let Some(slot) = by_issue.get_mut(&issue_id) else {
            continue;
        };
        let timestamp: String = row.try_get("timestamp")?;
        let user_key: Option<String> = row.try_get("user_key")?;
        let Some(parsed) = parse_time(&timestamp) else {
            continue;
        };
        if let Some(index) = bucket_index(parsed, Utc::now() - day, 24) {
            slot.0[index] += 1;
        }
        if let Some(index) = bucket_index(parsed, Utc::now() - Duration::days(14), 14) {
            slot.1[index] += 1;
        }
        let in_period = period_start.map(|start| parsed >= start).unwrap_or(true);
        if in_period {
            slot.2 += 1;
            if let Some(user_key) = user_key.filter(|value| !value.is_empty()) {
                slot.3.insert(user_key);
            }
        }
    }
    for issue in issues.iter_mut() {
        if let Some((trend24h, trend14d, period_events, users)) = by_issue.remove(&issue.id) {
            issue.trend24h = trend24h;
            issue.trend14d = trend14d;
            if !since.is_empty() {
                issue.period_event_count = period_events;
            }
            issue.user_count = users.len() as i64;
        }
    }
    Ok(())
}

fn sort_issues(issues: &mut [IssueRow], sort: &str) {
    match sort {
        "age" | "first_seen" => {
            issues.sort_by(|left, right| left.first_seen.cmp(&right.first_seen))
        }
        "events" => {
            issues.sort_by(|left, right| right.period_event_count.cmp(&left.period_event_count))
        }
        "users" => issues.sort_by(|left, right| right.user_count.cmp(&left.user_count)),
        _ => issues.sort_by(|left, right| right.last_seen.cmp(&left.last_seen)),
    }
}

fn since_bound(spec: &str) -> String {
    since_time(spec)
        .map(|time| time.to_rfc3339_opts(SecondsFormat::Millis, true))
        .unwrap_or_default()
}

fn since_time(spec: &str) -> Option<DateTime<Utc>> {
    let now = Utc::now();
    match spec {
        "1h" => Some(now - Duration::hours(1)),
        "24h" => Some(now - Duration::hours(24)),
        "7d" => Some(now - Duration::days(7)),
        "14d" => Some(now - Duration::days(14)),
        "30d" => Some(now - Duration::days(30)),
        _ => None,
    }
}

fn bucket_count(spec: &str) -> usize {
    match spec {
        "1h" => 12,
        "24h" => 24,
        "7d" => 7,
        "30d" => 30,
        _ => 14,
    }
}

fn bucket_index(timestamp: DateTime<Utc>, start: DateTime<Utc>, count: usize) -> Option<usize> {
    if count == 0 || timestamp < start {
        return None;
    }
    let span = Utc::now() - start;
    let width = (span.num_milliseconds().max(1) as f64) / count as f64;
    let offset = (timestamp - start).num_milliseconds().max(0) as f64;
    Some(((offset / width) as usize).min(count - 1))
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|time| time.with_timezone(&Utc))
}

fn priority_for(level: &str) -> String {
    match level {
        "fatal" => "high",
        "error" => "medium",
        _ => "low",
    }
    .to_owned()
}

fn short_prefix(slug: &str) -> String {
    let mut prefix: String = slug
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(16)
        .collect();
    prefix.make_ascii_uppercase();
    if prefix.is_empty() {
        "ISSUE".to_owned()
    } else {
        prefix
    }
}

fn event_unhandled(event: &NormalizedEvent) -> bool {
    event
        .exceptions
        .iter()
        .any(|exception| exception.handled == Some(false))
}

fn user_key_of(event: &NormalizedEvent) -> Option<String> {
    for key in ["id", "email", "username", "ip_address"] {
        if let Some(value) = event.user.get(key).and_then(|item| item.as_str()) {
            let value = value.trim();
            if !value.is_empty() && value != "{{auto}}" {
                return Some(value.to_owned());
            }
        }
    }
    None
}
