use std::net::IpAddr;
use std::sync::OnceLock;

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::http::{Method, Response, StatusCode, Uri};
use axum::response::IntoResponse;
use axum_extra::extract::cookie::CookieJar;
use glimwatch_insights::{
    Bounds, EventPayload, Incoming, LINK_SECTIONS, SITE_SECTIONS, clean_color, clean_destination,
    clean_host, clean_name, clean_public_slug, clean_slug, clean_vitals, clip, flatten, now_secs,
    parse_page, parse_user_agent, percentile, referrer_domain, same_site, sections_json,
    session_id, slugify,
};
use glimwatch_platform::{Database, Id, generate_token};
use http_body_util::BodyExt;
use isbot::Bots;
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;

use super::geo;
use super::record;
use crate::error::AppError;
use crate::routes::require_user;
use crate::state::AppState;

const SCRIPT: &str = include_str!("tracker.js");

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/sites", axum::routing::get(list_sites).post(create_site))
        .route(
            "/sites/{id}",
            axum::routing::get(get_site)
                .put(update_site)
                .delete(delete_site),
        )
        .route("/sites/{id}/analytics", axum::routing::delete(reset_site))
        .route("/sites/{id}/public", axum::routing::put(update_site_public))
        .route("/sites/{id}/overview", axum::routing::get(site_overview))
        .route(
            "/sites/{id}/breakdown/{dimension}",
            axum::routing::get(site_breakdown),
        )
        .route("/sites/{id}/realtime", axum::routing::get(site_realtime))
        .route("/sites/{id}/vitals", axum::routing::get(site_vitals))
        .route("/sites/{id}/events", axum::routing::get(site_events))
        .route(
            "/sites/{id}/events/{name}/properties",
            axum::routing::get(event_properties),
        )
        .route("/sites/{id}/visitors", axum::routing::get(site_visitors))
        .route(
            "/sites/{id}/visitors/{visitor_id}",
            axum::routing::get(visitor_detail),
        )
        .route(
            "/domains",
            axum::routing::get(list_domains).post(create_domain),
        )
        .route(
            "/domains/{id}",
            axum::routing::put(update_domain).delete(delete_domain),
        )
        .route("/links", axum::routing::get(list_links).post(create_link))
        .route(
            "/links/{id}",
            axum::routing::get(get_link)
                .put(update_link)
                .delete(delete_link),
        )
        .route("/links/{id}/public", axum::routing::put(update_link_public))
        .route("/links/{id}/overview", axum::routing::get(link_overview))
        .route(
            "/links/{id}/breakdown/{dimension}",
            axum::routing::get(link_breakdown),
        )
        .route(
            "/public/sites/{slug}/overview",
            axum::routing::get(public_site_overview),
        )
        .route(
            "/public/sites/{slug}/{dimension}",
            axum::routing::get(public_site_dimension),
        )
        .route(
            "/public/links/{slug}/overview",
            axum::routing::get(public_link_overview),
        )
        .route(
            "/public/links/{slug}/{dimension}",
            axum::routing::get(public_link_dimension),
        )
        .route("/receive", axum::routing::post(receive).options(preflight))
}

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/data.js", axum::routing::get(script).options(preflight))
        .route(
            "/go/{*slug}",
            axum::routing::get(short_link).head(short_link),
        )
}

pub async fn fallback(
    State(state): State<AppState>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response<Body> {
    if matches!(method, Method::GET | Method::HEAD)
        && let Some(response) = host_redirect(&state, &method, &uri, &headers).await
    {
        return response;
    }
    crate::static_assets::spa(State(state), method, uri).await
}

async fn preflight() -> Response<Body> {
    with_cors(StatusCode::NO_CONTENT.into_response())
}

async fn script() -> Response<Body> {
    let mut response = (StatusCode::OK, SCRIPT).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/javascript; charset=utf-8"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    with_cors(response)
}

async fn receive(State(state): State<AppState>, request: Request) -> Response<Body> {
    let headers = request.headers().clone();
    let peer = request
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|info| info.0.ip());
    let ip = client_ip(&headers, state.config.proxy_hops, peer);
    if !state.collector_limit.allow(&ip) {
        return with_cors(AppError::TooManyRequests.into_response());
    }
    let bytes = match request.into_body().collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return with_cors(AppError::field("body", "Unreadable body.").into_response()),
    };
    if bytes.len() > 16 * 1024 {
        return with_cors(AppError::field("body", "Event is larger than 16 KiB.").into_response());
    }
    let payload: EventPayload = match serde_json::from_slice(&bytes) {
        Ok(payload) => payload,
        Err(_) => {
            return with_cors(AppError::field("body", "Event JSON is invalid.").into_response());
        }
    };
    match accept_event(&state, &headers, &ip, payload).await {
        Ok(status) => with_cors(status.into_response()),
        Err(error) => with_cors(error.into_response()),
    }
}

async fn accept_event(
    state: &AppState,
    headers: &HeaderMap,
    ip: &str,
    payload: EventPayload,
) -> Result<StatusCode, AppError> {
    if payload.kind.is_empty()
        || payload.kind.len() > 64
        || !payload
            .kind
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(AppError::field("type", "Invalid event type."));
    }
    let site = sqlx::query("SELECT id, domain FROM insight_sites WHERE public_id = $1")
        .bind(&payload.site)
        .fetch_optional(state.db.pool())
        .await?;
    let Some(site) = site else {
        return Err(AppError::field("site", "Unknown site."));
    };
    let site_id: String = site.try_get("id")?;
    let domain: String = site.try_get("domain")?;
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if ua.is_empty() || bots().is_bot(ua) {
        return Ok(StatusCode::ACCEPTED);
    }
    let page = payload.url.as_deref().and_then(parse_page);
    let referrer = payload
        .referrer
        .as_deref()
        .and_then(referrer_domain)
        .filter(|host| !same_site(host, &domain));
    let performance = payload.kind == "performance";
    let vitals = payload
        .vitals
        .filter(|_| performance)
        .map(clean_vitals)
        .unwrap_or_default();
    let data = if performance {
        Vec::new()
    } else {
        payload.data.as_ref().map(flatten).unwrap_or_default()
    };
    let metadata = payload
        .data
        .as_ref()
        .filter(|_| !performance)
        .map(Value::to_string)
        .filter(|value| value.len() <= 4096);
    let now = now_secs();
    let place = geo::locate(state.geo.as_deref(), headers, state.config.proxy_hops, ip);
    let (browser, os, device) = parse_user_agent(ua);
    record::record(
        &state.db,
        Incoming {
            site_id: Some(site_id.clone()),
            timestamp: now,
            event_type: payload.kind,
            session_id: Some(session_id(&state.insights_secret, &site_id, ip, ua, now)),
            pathname: page.as_ref().map(|page| page.path.clone()),
            url_query: page.as_ref().and_then(|page| page.query.clone()),
            title: payload
                .title
                .map(|title| clip(title.trim(), 500))
                .filter(|title| !title.is_empty()),
            hostname: payload
                .hostname
                .map(|host| clip(&host.to_lowercase(), 100))
                .filter(|host| !host.is_empty()),
            utm: page.map(|page| page.utm).unwrap_or_default(),
            referrer_domain: referrer,
            country: place.country,
            region: place.region,
            city: place.city,
            browser,
            os,
            device,
            screen: payload
                .screen
                .map(|value| clip(&value, 11))
                .filter(|value| !value.is_empty()),
            language: payload
                .language
                .map(|value| clip(&value, 35))
                .filter(|value| !value.is_empty()),
            metadata,
            vitals,
            data,
            ..Incoming::default()
        },
    )
    .await?;
    Ok(StatusCode::ACCEPTED)
}

async fn short_link(
    State(state): State<AppState>,
    method: Method,
    Path(slug): Path<String>,
    headers: HeaderMap,
    request: Request,
) -> Response<Body> {
    let _ = request;
    resolve_slug(&state, method, &headers, None, slug.trim_matches('/')).await
}

async fn host_redirect(
    state: &AppState,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Option<Response<Body>> {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| clean_host(value).ok())?;
    let app = origin_host(&state.config.public_origin);
    if app.as_deref() == Some(host.as_str()) {
        return None;
    }
    let row = sqlx::query("SELECT id FROM insight_domains WHERE hostname = $1 AND enabled = 1")
        .bind(&host)
        .fetch_optional(state.db.pool())
        .await
        .ok()
        .flatten()?;
    let _ = row;
    let slug = uri.path().trim_matches('/');
    Some(resolve_slug(state, method.clone(), headers, Some(&host), slug).await)
}

async fn resolve_slug(
    state: &AppState,
    method: Method,
    headers: &HeaderMap,
    host: Option<&str>,
    slug: &str,
) -> Response<Body> {
    let rows = if let Some(host) = host {
        sqlx::query(
            "SELECT l.id, l.destination, l.redirect_type FROM insight_links l \
             JOIN insight_domains d ON d.id = l.domain_id \
             WHERE d.hostname = $1 AND l.slug = $2 AND d.enabled = 1 AND l.enabled = 1",
        )
        .bind(host)
        .bind(slug)
        .fetch_all(state.db.pool())
        .await
    } else {
        sqlx::query(
            "SELECT l.id, l.destination, l.redirect_type FROM insight_links l \
             JOIN insight_domains d ON d.id = l.domain_id \
             WHERE l.slug = $1 AND d.enabled = 1 AND l.enabled = 1",
        )
        .bind(slug)
        .fetch_all(state.db.pool())
        .await
    };
    let rows = match rows {
        Ok(rows) => rows,
        Err(error) => {
            tracing::error!(%error, "link lookup failed");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };
    if rows.len() != 1 {
        return StatusCode::NOT_FOUND.into_response();
    }
    let row = &rows[0];
    let id: String = row.try_get("id").unwrap_or_default();
    let destination: String = row.try_get("destination").unwrap_or_default();
    let kind: String = row.try_get("redirect_type").unwrap_or_default();
    let ua = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    if method != Method::HEAD && !ua.is_empty() && !bots().is_bot(ua) {
        let peer = None;
        let ip = client_ip(headers, state.config.proxy_hops, peer);
        let (browser, os, device) = parse_user_agent(ua);
        let now = now_secs();
        let place = geo::locate(state.geo.as_deref(), headers, state.config.proxy_hops, &ip);
        let click = Incoming {
            link_id: Some(id.clone()),
            timestamp: now,
            event_type: "click".into(),
            session_id: Some(session_id(&state.insights_secret, &id, &ip, ua, now)),
            referrer_domain: headers
                .get(header::REFERER)
                .and_then(|value| value.to_str().ok())
                .and_then(referrer_domain),
            country: place.country,
            region: place.region,
            city: place.city,
            browser,
            os,
            device,
            ..Incoming::default()
        };
        if let Err(error) = record::record(&state.db, click).await {
            tracing::warn!(%error, "click was not recorded");
        }
    }
    redirect_to(&destination, kind == "permanent")
}

fn redirect_to(destination: &str, permanent: bool) -> Response<Body> {
    let Ok(location) = HeaderValue::from_str(destination) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let status = if permanent {
        StatusCode::PERMANENT_REDIRECT
    } else {
        StatusCode::TEMPORARY_REDIRECT
    };
    (
        status,
        [
            (header::LOCATION, location),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
        ],
    )
        .into_response()
}

#[derive(Deserialize)]
struct SiteBody {
    name: String,
    domain: String,
    color: Option<String>,
}

#[derive(Deserialize)]
struct DomainBody {
    hostname: String,
    note: Option<String>,
    enabled: Option<bool>,
    site_id: Option<String>,
}

#[derive(Deserialize)]
struct LinkBody {
    domain_id: String,
    slug: String,
    destination: String,
    redirect_type: Option<String>,
    enabled: Option<bool>,
}

#[derive(Deserialize)]
struct PublicBody {
    enabled: bool,
    slug: String,
    threshold: i64,
    sections: Vec<String>,
}

#[derive(Deserialize)]
struct RangeQuery {
    from: Option<i64>,
    to: Option<i64>,
}

async fn list_sites(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let rows = sqlx::query(
        "SELECT id, public_id, name, domain, color, public_slug, public_enabled, public_threshold, public_sections, created_at \
         FROM insight_sites ORDER BY created_at",
    )
    .fetch_all(state.db.pool())
    .await?;
    Ok(Json(Value::Array(
        rows.iter().map(site_json).collect::<Result<Vec<_>, _>>()?,
    )))
}

async fn create_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<SiteBody>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    require_user(&state, &jar).await?;
    let id = Id::new().to_string();
    let public_id = public_token();
    let name = field("name", clean_name(&body.name))?;
    let domain = field("domain", clean_host(&body.domain))?;
    let color = match body.color {
        Some(color) => field("color", clean_color(&color))?,
        None => "#4f46e5".to_owned(),
    };
    let slug = format!("{}-{}", slugify(&name), &id[id.len().saturating_sub(6)..]);
    let slug = field("name", clean_public_slug(&slug))
        .unwrap_or_else(|_| format!("site-{}", &id[id.len().saturating_sub(6)..]));
    sqlx::query(
        "INSERT INTO insight_sites (id, public_id, name, domain, color, public_slug, public_sections, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(&id)
    .bind(&public_id)
    .bind(&name)
    .bind(&domain)
    .bind(&color)
    .bind(&slug)
    .bind(sections_json(SITE_SECTIONS))
    .bind(now_secs())
    .execute(state.db.pool())
    .await
    .map_err(map_db)?;
    Ok((StatusCode::CREATED, Json(fetch_site(&state.db, &id).await?)))
}

async fn get_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(fetch_site(&state.db, &id).await?))
}

async fn update_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<SiteBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let name = field("name", clean_name(&body.name))?;
    let domain = field("domain", clean_host(&body.domain))?;
    let color = match body.color {
        Some(color) => field("color", clean_color(&color))?,
        None => "#4f46e5".to_owned(),
    };
    let changed =
        sqlx::query("UPDATE insight_sites SET name = $1, domain = $2, color = $3 WHERE id = $4")
            .bind(name)
            .bind(domain)
            .bind(color)
            .bind(&id)
            .execute(state.db.pool())
            .await
            .map_err(map_db)?
            .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(fetch_site(&state.db, &id).await?))
}

async fn delete_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    let changed = sqlx::query("DELETE FROM insight_sites WHERE id = $1")
        .bind(id)
        .execute(state.db.pool())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn reset_site(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    for sql in [
        "DELETE FROM insight_events WHERE site_id = $1",
        "DELETE FROM insight_visits WHERE site_id = $1",
        "DELETE FROM insight_daily WHERE site_id = $1",
        "DELETE FROM insight_dimension_daily WHERE site_id = $1",
        "DELETE FROM insight_site_sessions WHERE site_id = $1",
    ] {
        sqlx::query(sql).bind(&id).execute(state.db.pool()).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn update_site_public(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<PublicBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    save_public(
        &state.db,
        "UPDATE insight_sites SET public_enabled = $1, public_slug = $2, public_threshold = $3, public_sections = $4 WHERE id = $5",
        &id,
        &body,
        SITE_SECTIONS,
    )
    .await?;
    Ok(Json(fetch_site(&state.db, &id).await?))
}

async fn site_overview(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    Ok(Json(site_totals(&state.db, &id, range).await?))
}

async fn site_breakdown(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((id, dimension)): Path<(String, String)>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    Ok(Json(
        json!({ "rows": breakdown(&state.db, "site_id", &id, &dimension, range, "pageview").await? }),
    ))
}

async fn site_realtime(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    let now = now_secs();
    let current: i64 = scalar(
        &state.db,
        "SELECT CAST(COUNT(DISTINCT session_id) AS BIGINT) AS n FROM insight_events \
         WHERE site_id = $1 AND timestamp >= $2 AND event_type <> 'performance'",
        &id,
        now - 5 * 60,
        0,
    )
    .await?;
    let series_rows = sqlx::query(
        "SELECT (timestamp - timestamp % 60) AS minute, CAST(COUNT(*) AS BIGINT) AS views \
         FROM insight_events WHERE site_id = $1 AND event_type = 'pageview' AND timestamp >= $2 \
         GROUP BY minute ORDER BY minute",
    )
    .bind(&id)
    .bind(now - 30 * 60)
    .fetch_all(state.db.pool())
    .await?;
    let activity = sqlx::query(
        "SELECT timestamp, event_type, pathname, country, browser, device FROM insight_events \
         WHERE site_id = $1 AND event_type <> 'performance' ORDER BY timestamp DESC LIMIT 20",
    )
    .bind(&id)
    .fetch_all(state.db.pool())
    .await?;
    let mut activity_json = Vec::new();
    for row in &activity {
        activity_json.push(json!({
            "timestamp": row.try_get::<i64, _>("timestamp")?,
            "type": row.try_get::<String, _>("event_type")?,
            "path": row.try_get::<Option<String>, _>("pathname")?,
            "country": row.try_get::<Option<String>, _>("country")?,
            "browser": row.try_get::<Option<String>, _>("browser")?,
            "device": row.try_get::<Option<String>, _>("device")?,
        }));
    }
    let mut series = Vec::new();
    for row in &series_rows {
        series.push(json!({
            "minute": row.try_get::<i64, _>("minute")?,
            "views": row.try_get::<i64, _>("views")?,
        }));
    }
    Ok(Json(json!({
        "current_visitors": current,
        "series": series,
        "activity": activity_json,
    })))
}

async fn site_vitals(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let rows = sqlx::query(
        "SELECT device, lcp, inp, cls, fcp, ttfb FROM insight_events \
         WHERE site_id = $1 AND event_type = 'performance' AND timestamp >= $2 AND timestamp <= $3 \
         ORDER BY timestamp DESC LIMIT 5000",
    )
    .bind(&id)
    .bind(bounds.from)
    .bind(bounds.to)
    .fetch_all(state.db.pool())
    .await?;
    let mut buckets: std::collections::BTreeMap<(String, String), Vec<f64>> =
        std::collections::BTreeMap::new();
    for row in &rows {
        let device: String = row
            .try_get::<Option<String>, _>("device")?
            .unwrap_or_else(|| "Desktop".into());
        for metric in ["lcp", "inp", "cls", "fcp", "ttfb"] {
            let value: Option<f64> = row.try_get(metric)?;
            if let Some(value) = value {
                buckets
                    .entry((metric.to_owned(), device.clone()))
                    .or_default()
                    .push(value);
            }
        }
    }
    let mut output = Vec::new();
    for ((metric, device), mut values) in buckets {
        values.sort_by(|left, right| left.total_cmp(right));
        output.push(json!({
            "metric": metric,
            "device": device,
            "p75": percentile(&values, 0.75),
            "samples": values.len() as i64,
        }));
    }
    Ok(Json(json!({ "rows": output })))
}

async fn site_events(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_site(&state.db, &id).await?;
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let rows = sqlx::query(
        "SELECT event_type AS name, CAST(COUNT(*) AS BIGINT) AS count, \
         CAST(COUNT(DISTINCT session_id) AS BIGINT) AS visitors, CAST(MAX(timestamp) AS BIGINT) AS last_seen \
         FROM insight_events WHERE site_id = $1 AND timestamp >= $2 AND timestamp <= $3 \
         AND event_type NOT IN ('pageview', 'performance') \
         GROUP BY event_type ORDER BY count DESC LIMIT 50",
    )
    .bind(&id)
    .bind(bounds.from)
    .bind(bounds.to)
    .fetch_all(state.db.pool())
    .await?;
    let mut output = Vec::new();
    let mut total = 0;
    for row in &rows {
        let count: i64 = row.try_get("count")?;
        total += count;
        output.push(json!({
            "name": row.try_get::<String, _>("name")?,
            "count": count,
            "visitors": row.try_get::<i64, _>("visitors")?,
            "last_seen": row.try_get::<i64, _>("last_seen")?,
        }));
    }
    Ok(Json(json!({ "total": total, "rows": output })))
}

async fn event_properties(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((id, name)): Path<(String, String)>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let rows = sqlx::query(
        "SELECT key, string_value, CAST(COUNT(*) AS BIGINT) AS count FROM insight_event_data \
         WHERE site_id = $1 AND event_name = $2 AND timestamp >= $3 AND timestamp <= $4 \
         GROUP BY key, string_value ORDER BY count DESC LIMIT 200",
    )
    .bind(&id)
    .bind(&name)
    .bind(bounds.from)
    .bind(bounds.to)
    .fetch_all(state.db.pool())
    .await?;
    let mut grouped: std::collections::BTreeMap<String, Vec<Value>> =
        std::collections::BTreeMap::new();
    for row in &rows {
        grouped
            .entry(row.try_get::<String, _>("key")?)
            .or_default()
            .push(json!({
                "label": row.try_get::<String, _>("string_value")?,
                "count": row.try_get::<i64, _>("count")?,
            }));
    }
    let rows = grouped
        .into_iter()
        .map(|(key, values)| json!({ "key": key, "values": values }))
        .collect::<Vec<_>>();
    Ok(Json(json!({ "rows": rows })))
}

async fn site_visitors(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let rows = sqlx::query(
        "SELECT session_id AS id, CAST(MIN(timestamp) AS BIGINT) AS first_seen, CAST(MAX(timestamp) AS BIGINT) AS last_seen, \
         CAST(SUM(CASE WHEN event_type = 'pageview' THEN 1 ELSE 0 END) AS BIGINT) AS views, \
         CAST(SUM(CASE WHEN event_type NOT IN ('pageview', 'performance') THEN 1 ELSE 0 END) AS BIGINT) AS events \
         FROM insight_events WHERE site_id = $1 AND session_id IS NOT NULL AND timestamp >= $2 AND timestamp <= $3 \
         GROUP BY session_id ORDER BY last_seen DESC LIMIT 100",
    )
    .bind(&id)
    .bind(bounds.from)
    .bind(bounds.to)
    .fetch_all(state.db.pool())
    .await?;
    let mut output = Vec::new();
    for row in &rows {
        output.push(json!({
            "id": row.try_get::<String, _>("id")?,
            "first_seen": row.try_get::<i64, _>("first_seen")?,
            "last_seen": row.try_get::<i64, _>("last_seen")?,
            "views": row.try_get::<i64, _>("views")?,
            "events": row.try_get::<i64, _>("events")?,
        }));
    }
    Ok(Json(json!({ "rows": output })))
}

async fn visitor_detail(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((id, visitor_id)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let rows = sqlx::query(
        "SELECT timestamp, event_type, pathname, title, metadata, country, region, city, browser, os, device, screen, language \
         FROM insight_events WHERE site_id = $1 AND session_id = $2 ORDER BY timestamp",
    )
    .bind(&id)
    .bind(&visitor_id)
    .fetch_all(state.db.pool())
    .await?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }
    let mut activity = Vec::new();
    for row in &rows {
        activity.push(json!({
            "timestamp": row.try_get::<i64, _>("timestamp")?,
            "type": row.try_get::<String, _>("event_type")?,
            "path": row.try_get::<Option<String>, _>("pathname")?,
            "title": row.try_get::<Option<String>, _>("title")?,
            "metadata": row.try_get::<Option<String>, _>("metadata")?,
        }));
    }
    let first = &rows[0];
    let last = rows.last().unwrap_or(first);
    Ok(Json(json!({
        "visitor": {
            "id": visitor_id,
            "first_seen": first.try_get::<i64, _>("timestamp")?,
            "last_seen": last.try_get::<i64, _>("timestamp")?,
            "views": rows.iter().filter(|row| row.try_get::<String, _>("event_type").ok().as_deref() == Some("pageview")).count(),
            "country": last.try_get::<Option<String>, _>("country")?,
            "region": last.try_get::<Option<String>, _>("region")?,
            "city": last.try_get::<Option<String>, _>("city")?,
            "browser": last.try_get::<Option<String>, _>("browser")?,
            "os": last.try_get::<Option<String>, _>("os")?,
            "device": last.try_get::<Option<String>, _>("device")?,
            "screen": last.try_get::<Option<String>, _>("screen")?,
            "language": last.try_get::<Option<String>, _>("language")?,
        },
        "activity": activity,
    })))
}

async fn list_domains(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let rows = sqlx::query(
        "SELECT id, site_id, hostname, note, enabled, created_at FROM insight_domains ORDER BY hostname",
    )
    .fetch_all(state.db.pool())
    .await?;
    let mut output = Vec::new();
    for row in &rows {
        output.push(domain_json(row)?);
    }
    Ok(Json(Value::Array(output)))
}

async fn create_domain(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<DomainBody>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    require_user(&state, &jar).await?;
    let id = Id::new().to_string();
    let hostname = field("hostname", clean_host(&body.hostname))?;
    let note = body.note.unwrap_or_default();
    if note.chars().count() > 200 {
        return Err(AppError::field("note", "Use at most 200 characters."));
    }
    sqlx::query(
        "INSERT INTO insight_domains (id, site_id, hostname, note, enabled, created_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(&id)
    .bind(&body.site_id)
    .bind(&hostname)
    .bind(&note)
    .bind(i64::from(body.enabled.unwrap_or(true)))
    .bind(now_secs())
    .execute(state.db.pool())
    .await
    .map_err(map_db)?;
    Ok((
        StatusCode::CREATED,
        Json(fetch_domain(&state.db, &id).await?),
    ))
}

async fn update_domain(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<DomainBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let hostname = field("hostname", clean_host(&body.hostname))?;
    let changed = sqlx::query(
        "UPDATE insight_domains SET hostname = $1, note = $2, enabled = $3, site_id = $4 WHERE id = $5",
    )
    .bind(hostname)
    .bind(body.note.unwrap_or_default())
    .bind(i64::from(body.enabled.unwrap_or(true)))
    .bind(&body.site_id)
    .bind(&id)
    .execute(state.db.pool())
    .await
    .map_err(map_db)?
    .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(fetch_domain(&state.db, &id).await?))
}

async fn delete_domain(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    let changed = sqlx::query("DELETE FROM insight_domains WHERE id = $1")
        .bind(id)
        .execute(state.db.pool())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn list_links(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(Value::Array(link_rows(&state.db, None).await?)))
}

async fn create_link(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(body): Json<LinkBody>,
) -> Result<(StatusCode, Json<Value>), AppError> {
    require_user(&state, &jar).await?;
    let id = insert_link(&state.db, &body).await?;
    Ok((StatusCode::CREATED, Json(fetch_link(&state.db, &id).await?)))
}

async fn get_link(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    Ok(Json(fetch_link(&state.db, &id).await?))
}

async fn update_link(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<LinkBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    let slug = field("slug", clean_slug(&body.slug))?;
    let destination = field("destination", clean_destination(&body.destination))?;
    let redirect_type = redirect_type(body.redirect_type.as_deref())?;
    let changed = sqlx::query(
        "UPDATE insight_links SET domain_id = $1, slug = $2, destination = $3, redirect_type = $4, enabled = $5 WHERE id = $6",
    )
    .bind(&body.domain_id)
    .bind(slug)
    .bind(destination)
    .bind(redirect_type)
    .bind(i64::from(body.enabled.unwrap_or(true)))
    .bind(&id)
    .execute(state.db.pool())
    .await
    .map_err(map_db)?
    .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(fetch_link(&state.db, &id).await?))
}

async fn delete_link(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    require_user(&state, &jar).await?;
    let changed = sqlx::query("DELETE FROM insight_links WHERE id = $1")
        .bind(id)
        .execute(state.db.pool())
        .await?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn update_link_public(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Json(body): Json<PublicBody>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    save_public(
        &state.db,
        "UPDATE insight_links SET public_enabled = $1, public_slug = $2, public_threshold = $3, public_sections = $4 WHERE id = $5",
        &id,
        &body,
        LINK_SECTIONS,
    )
    .await?;
    Ok(Json(fetch_link(&state.db, &id).await?))
}

async fn link_overview(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(id): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_link(&state.db, &id).await?;
    Ok(Json(link_totals(&state.db, &id, range, true).await?))
}

async fn link_breakdown(
    State(state): State<AppState>,
    jar: CookieJar,
    Path((id, dimension)): Path<(String, String)>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    require_user(&state, &jar).await?;
    fetch_link(&state.db, &id).await?;
    Ok(Json(
        json!({ "rows": breakdown(&state.db, "link_id", &id, &dimension, range, "click").await? }),
    ))
}

async fn public_site_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    limit_public(&state, &headers)?;
    let resource = public_site(&state.db, &slug).await?;
    let totals = site_totals(&state.db, &resource.id, range).await?;
    Ok(Json(public_overview(
        resource,
        totals,
        "pageviews",
        "views",
    )))
}

async fn public_link_overview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slug): Path<String>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    limit_public(&state, &headers)?;
    let resource = public_link(&state.db, &slug).await?;
    let totals = link_totals(&state.db, &resource.id, range, false).await?;
    Ok(Json(public_overview(resource, totals, "clicks", "clicks")))
}

async fn public_site_dimension(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slug, dimension)): Path<(String, String)>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    limit_public(&state, &headers)?;
    let resource = public_site(&state.db, &slug).await?;
    Ok(Json(
        public_dimension(
            &state.db,
            &resource,
            "insight_dimension_daily",
            "site_id",
            &dimension,
            range,
        )
        .await?,
    ))
}

async fn public_link_dimension(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slug, dimension)): Path<(String, String)>,
    Query(range): Query<RangeQuery>,
) -> Result<Json<Value>, AppError> {
    limit_public(&state, &headers)?;
    let resource = public_link(&state.db, &slug).await?;
    Ok(Json(
        public_dimension(
            &state.db,
            &resource,
            "insight_link_dimension_daily",
            "link_id",
            &dimension,
            range,
        )
        .await?,
    ))
}

struct PublicResource {
    id: String,
    name: String,
    subtitle: String,
    threshold: i64,
    sections: Vec<String>,
}

impl PublicResource {
    fn shows(&self, key: &str) -> bool {
        self.sections.iter().any(|section| section == key)
    }
}

async fn site_totals(db: &Database, id: &str, range: RangeQuery) -> Result<Value, AppError> {
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let row = sqlx::query(
        "SELECT CAST(COALESCE(SUM(pageviews), 0) AS BIGINT) AS pageviews, \
         CAST(COALESCE(SUM(visitors), 0) AS BIGINT) AS visitors, \
         CAST(COALESCE(SUM(events), 0) AS BIGINT) AS events, \
         CAST(COALESCE(SUM(visits), 0) AS BIGINT) AS visits, \
         CAST(COALESCE(SUM(bounces), 0) AS BIGINT) AS bounces, \
         CAST(COALESCE(SUM(total_time), 0) AS BIGINT) AS total_time \
         FROM insight_daily WHERE site_id = $1 AND day BETWEEN $2 AND $3",
    )
    .bind(id)
    .bind(bounds.first_day)
    .bind(bounds.last_day)
    .fetch_one(db.pool())
    .await?;
    let series_rows = sqlx::query(
        "SELECT day, pageviews, visitors, events, visits, bounces, total_time FROM insight_daily \
         WHERE site_id = $1 AND day BETWEEN $2 AND $3 ORDER BY day",
    )
    .bind(id)
    .bind(bounds.first_day)
    .bind(bounds.last_day)
    .fetch_all(db.pool())
    .await?;
    let mut by_day = std::collections::BTreeMap::new();
    for row in &series_rows {
        by_day.insert(row.try_get::<i64, _>("day")?, row);
    }
    let mut series = Vec::new();
    let mut day = bounds.first_day;
    while day <= bounds.last_day {
        if let Some(row) = by_day.get(&day) {
            series.push(json!({
                "day": day,
                "views": row.try_get::<i64, _>("pageviews")?,
                "visitors": row.try_get::<i64, _>("visitors")?,
                "events": row.try_get::<i64, _>("events")?,
                "visits": row.try_get::<i64, _>("visits")?,
                "bounces": row.try_get::<i64, _>("bounces")?,
                "time": row.try_get::<i64, _>("total_time")?,
            }));
        } else {
            series.push(json!({"day": day, "views": 0, "visitors": 0, "events": 0, "visits": 0, "bounces": 0, "time": 0}));
        }
        day += 86_400;
    }
    Ok(json!({
        "pageviews": row.try_get::<i64, _>("pageviews")?,
        "visitors": row.try_get::<i64, _>("visitors")?,
        "events": row.try_get::<i64, _>("events")?,
        "visits": row.try_get::<i64, _>("visits")?,
        "bounces": row.try_get::<i64, _>("bounces")?,
        "total_time": row.try_get::<i64, _>("total_time")?,
        "from": bounds.from,
        "to": bounds.to,
        "unit": "day",
        "series": series,
    }))
}

async fn link_totals(
    db: &Database,
    id: &str,
    range: RangeQuery,
    named: bool,
) -> Result<Value, AppError> {
    let _ = named;
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let row = sqlx::query(
        "SELECT CAST(COALESCE(SUM(clicks), 0) AS BIGINT) AS clicks, \
         CAST(COALESCE(SUM(visitors), 0) AS BIGINT) AS visitors \
         FROM insight_link_daily WHERE link_id = $1 AND day BETWEEN $2 AND $3",
    )
    .bind(id)
    .bind(bounds.first_day)
    .bind(bounds.last_day)
    .fetch_one(db.pool())
    .await?;
    let series_rows = sqlx::query(
        "SELECT day, clicks, visitors FROM insight_link_daily WHERE link_id = $1 AND day BETWEEN $2 AND $3 ORDER BY day",
    )
    .bind(id)
    .bind(bounds.first_day)
    .bind(bounds.last_day)
    .fetch_all(db.pool())
    .await?;
    let mut by_day = std::collections::BTreeMap::new();
    for row in &series_rows {
        by_day.insert(row.try_get::<i64, _>("day")?, row);
    }
    let mut series = Vec::new();
    let mut day = bounds.first_day;
    while day <= bounds.last_day {
        if let Some(row) = by_day.get(&day) {
            series.push(json!({
                "day": day,
                "clicks": row.try_get::<i64, _>("clicks")?,
                "visitors": row.try_get::<i64, _>("visitors")?,
            }));
        } else {
            series.push(json!({"day": day, "clicks": 0, "visitors": 0}));
        }
        day += 86_400;
    }
    Ok(json!({
        "clicks": row.try_get::<i64, _>("clicks")?,
        "visitors": row.try_get::<i64, _>("visitors")?,
        "from": bounds.from,
        "to": bounds.to,
        "unit": "day",
        "series": series,
    }))
}

async fn breakdown(
    db: &Database,
    column: &str,
    id: &str,
    dimension: &str,
    range: RangeQuery,
    event_type: &str,
) -> Result<Vec<Value>, AppError> {
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let sql = match dimension {
        "entry" | "exit" => {
            let order = if dimension == "entry" { "ASC" } else { "DESC" };
            format!(
                "SELECT value, CAST(COUNT(*) AS BIGINT) AS count, CAST(COUNT(DISTINCT session_id) AS BIGINT) AS visitors FROM ( \
                    SELECT pathname AS value, session_id, ROW_NUMBER() OVER (PARTITION BY visit_id ORDER BY timestamp {order}, id {order}) AS rn \
                    FROM insight_events WHERE {column} = $1 AND event_type = $2 AND visit_id IS NOT NULL \
                    AND timestamp >= $3 AND timestamp <= $4 \
                 ) ranked WHERE rn = 1 AND value IS NOT NULL GROUP BY value ORDER BY count DESC LIMIT 50"
            )
        }
        other => {
            let field = dimension_column(other).ok_or(AppError::NotFound)?;
            format!(
                "SELECT {field} AS value, CAST(COUNT(*) AS BIGINT) AS count, CAST(COUNT(DISTINCT session_id) AS BIGINT) AS visitors \
                 FROM insight_events WHERE {column} = $1 AND event_type = $2 AND timestamp >= $3 AND timestamp <= $4 AND {field} IS NOT NULL \
                 GROUP BY {field} ORDER BY count DESC LIMIT 50"
            )
        }
    };
    let rows = sqlx::query(&sql)
        .bind(id)
        .bind(event_type)
        .bind(bounds.from)
        .bind(bounds.to)
        .fetch_all(db.pool())
        .await?;
    let mut output = Vec::new();
    for row in &rows {
        output.push(json!({
            "label": row.try_get::<String, _>("value")?,
            "count": row.try_get::<i64, _>("count")?,
            "visitors": row.try_get::<i64, _>("visitors")?,
        }));
    }
    Ok(output)
}

fn dimension_column(name: &str) -> Option<&'static str> {
    Some(match name {
        "path" => "pathname",
        "referrer" => "referrer_domain",
        "country" => "country",
        "region" => "region",
        "city" => "city",
        "browser" => "browser",
        "os" => "os",
        "device" => "device",
        "screen" => "screen",
        "language" => "language",
        "title" => "title",
        "hostname" => "hostname",
        "query" => "url_query",
        "utm_source" => "utm_source",
        "utm_medium" => "utm_medium",
        "utm_campaign" => "utm_campaign",
        "utm_content" => "utm_content",
        "utm_term" => "utm_term",
        _ => return None,
    })
}

fn public_overview(
    resource: PublicResource,
    totals: Value,
    metric: &str,
    series_key: &str,
) -> Value {
    let show = |key: &str, value: &Value| {
        if resource.shows(key) {
            value.clone()
        } else {
            Value::Null
        }
    };
    let series = totals["series"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .map(|point| {
            json!({
                "day": point["day"].clone(),
                series_key: show(metric, &point[series_key]),
                "visitors": show("visitors", &point["visitors"]),
            })
        })
        .collect::<Vec<_>>();
    json!({
        "name": resource.name,
        "subtitle": resource.subtitle,
        "sections": resource.sections,
        metric: show(metric, &totals[metric]),
        "visitors": show("visitors", &totals["visitors"]),
        "visits": show("visitors", &totals["visits"]),
        "bounces": show("visitors", &totals["bounces"]),
        "total_time": show("visitors", &totals["total_time"]),
        "from": totals["from"].clone(),
        "to": totals["to"].clone(),
        "unit": "day",
        "series": series,
    })
}

async fn public_dimension(
    db: &Database,
    resource: &PublicResource,
    table: &str,
    id_column: &str,
    dimension: &str,
    range: RangeQuery,
) -> Result<Value, AppError> {
    const ALLOWED: [&str; 6] = ["path", "referrer", "country", "browser", "os", "device"];
    if !ALLOWED.contains(&dimension) || !resource.shows(dimension) {
        return Err(AppError::NotFound);
    }
    let bounds = Bounds::resolve(range.from, range.to, now_secs());
    let sql = format!(
        "SELECT value, CAST(SUM(count) AS BIGINT) AS total FROM {table} \
         WHERE {id_column} = $1 AND dimension = $2 AND day BETWEEN $3 AND $4 \
         GROUP BY value ORDER BY total DESC LIMIT 50"
    );
    let rows = sqlx::query(&sql)
        .bind(&resource.id)
        .bind(dimension)
        .bind(bounds.first_day)
        .bind(bounds.last_day)
        .fetch_all(db.pool())
        .await?;
    let mut other = 0;
    let mut output = Vec::new();
    for row in &rows {
        let count: i64 = row.try_get("total")?;
        if count < resource.threshold {
            other += count;
        } else {
            output.push(json!({"label": row.try_get::<String, _>("value")?, "count": count}));
        }
    }
    if other > 0 {
        output.push(json!({"label": "Other", "count": other}));
    }
    Ok(json!({ "rows": output }))
}

async fn public_site(db: &Database, slug: &str) -> Result<PublicResource, AppError> {
    let row = sqlx::query(
        "SELECT id, name, domain, public_threshold, public_sections FROM insight_sites \
         WHERE public_slug = $1 AND public_enabled = 1",
    )
    .bind(slug)
    .fetch_optional(db.pool())
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(PublicResource {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        subtitle: row.try_get("domain")?,
        threshold: row.try_get("public_threshold")?,
        sections: serde_json::from_str(&row.try_get::<String, _>("public_sections")?)
            .unwrap_or_default(),
    })
}

async fn public_link(db: &Database, slug: &str) -> Result<PublicResource, AppError> {
    let row = sqlx::query(
        "SELECT l.id, d.hostname, l.slug, l.public_threshold, l.public_sections FROM insight_links l \
         JOIN insight_domains d ON d.id = l.domain_id \
         WHERE l.public_slug = $1 AND l.public_enabled = 1",
    )
    .bind(slug)
    .fetch_optional(db.pool())
    .await?
    .ok_or(AppError::NotFound)?;
    let hostname: String = row.try_get("hostname")?;
    let slug: String = row.try_get("slug")?;
    Ok(PublicResource {
        id: row.try_get("id")?,
        name: format!("{hostname}/{slug}"),
        subtitle: "Short link".into(),
        threshold: row.try_get("public_threshold")?,
        sections: serde_json::from_str(&row.try_get::<String, _>("public_sections")?)
            .unwrap_or_default(),
    })
}

fn limit_public(state: &AppState, headers: &HeaderMap) -> Result<(), AppError> {
    let ip = client_ip(headers, state.config.proxy_hops, None);
    if state.public_limit.allow(&ip) {
        Ok(())
    } else {
        Err(AppError::TooManyRequests)
    }
}

async fn save_public(
    db: &Database,
    sql: &str,
    id: &str,
    body: &PublicBody,
    allowed: &[&str],
) -> Result<(), AppError> {
    if !(0..=100_000).contains(&body.threshold) {
        return Err(AppError::field(
            "threshold",
            "Threshold must be between 0 and 100000.",
        ));
    }
    if body
        .sections
        .iter()
        .any(|section| !allowed.contains(&section.as_str()))
    {
        return Err(AppError::field("sections", "Unknown section."));
    }
    let slug = field("slug", clean_public_slug(&body.slug))?;
    let changed = sqlx::query(sql)
        .bind(i64::from(body.enabled))
        .bind(slug)
        .bind(body.threshold)
        .bind(serde_json::to_string(&body.sections).unwrap_or_else(|_| "[]".into()))
        .bind(id)
        .execute(db.pool())
        .await
        .map_err(map_db)?
        .rows_affected();
    if changed == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

async fn insert_link(db: &Database, body: &LinkBody) -> Result<String, AppError> {
    let id = Id::new().to_string();
    let slug = field("slug", clean_slug(&body.slug))?;
    let destination = field("destination", clean_destination(&body.destination))?;
    let redirect_type = redirect_type(body.redirect_type.as_deref())?;
    let public_slug = format!("link-{}", &id[id.len().saturating_sub(8)..]);
    sqlx::query(
        "INSERT INTO insight_links (id, domain_id, slug, destination, redirect_type, enabled, public_slug, public_sections, created_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(&id)
    .bind(&body.domain_id)
    .bind(slug)
    .bind(destination)
    .bind(redirect_type)
    .bind(i64::from(body.enabled.unwrap_or(true)))
    .bind(public_slug)
    .bind(sections_json(LINK_SECTIONS))
    .bind(now_secs())
    .execute(db.pool())
    .await
    .map_err(map_db)?;
    Ok(id)
}

fn redirect_type(value: Option<&str>) -> Result<&'static str, AppError> {
    match value.unwrap_or("temporary") {
        "temporary" => Ok("temporary"),
        "permanent" => Ok("permanent"),
        _ => Err(AppError::field(
            "redirect_type",
            "Use temporary or permanent.",
        )),
    }
}

async fn fetch_site(db: &Database, id: &str) -> Result<Value, AppError> {
    let row = sqlx::query(
        "SELECT id, public_id, name, domain, color, public_slug, public_enabled, public_threshold, public_sections, created_at \
         FROM insight_sites WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?
    .ok_or(AppError::NotFound)?;
    site_json(&row)
}

fn site_json(row: &sqlx::any::AnyRow) -> Result<Value, AppError> {
    Ok(json!({
        "id": row.try_get::<String, _>("id")?,
        "public_id": row.try_get::<String, _>("public_id")?,
        "name": row.try_get::<String, _>("name")?,
        "domain": row.try_get::<String, _>("domain")?,
        "color": row.try_get::<String, _>("color")?,
        "public_slug": row.try_get::<String, _>("public_slug")?,
        "public_enabled": row.try_get::<i64, _>("public_enabled")? != 0,
        "public_threshold": row.try_get::<i64, _>("public_threshold")?,
        "public_sections": serde_json::from_str::<Vec<String>>(&row.try_get::<String, _>("public_sections")?).unwrap_or_default(),
        "created_at": row.try_get::<i64, _>("created_at")?,
    }))
}

async fn fetch_domain(db: &Database, id: &str) -> Result<Value, AppError> {
    let row = sqlx::query(
        "SELECT id, site_id, hostname, note, enabled, created_at FROM insight_domains WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db.pool())
    .await?
    .ok_or(AppError::NotFound)?;
    domain_json(&row)
}

fn domain_json(row: &sqlx::any::AnyRow) -> Result<Value, AppError> {
    Ok(json!({
        "id": row.try_get::<String, _>("id")?,
        "site_id": row.try_get::<Option<String>, _>("site_id")?,
        "hostname": row.try_get::<String, _>("hostname")?,
        "note": row.try_get::<String, _>("note")?,
        "enabled": row.try_get::<i64, _>("enabled")? != 0,
        "created_at": row.try_get::<i64, _>("created_at")?,
    }))
}

async fn link_rows(db: &Database, id: Option<&str>) -> Result<Vec<Value>, AppError> {
    let rows = if let Some(id) = id {
        sqlx::query(
            "SELECT l.id, l.domain_id, d.hostname, l.slug, l.destination, l.redirect_type, l.enabled, \
             l.public_slug, l.public_enabled, l.public_threshold, l.public_sections, l.created_at, \
             CAST(COALESCE(SUM(s.clicks), 0) AS BIGINT) AS total_clicks \
             FROM insight_links l JOIN insight_domains d ON d.id = l.domain_id \
             LEFT JOIN insight_link_daily s ON s.link_id = l.id \
             WHERE l.id = $1 GROUP BY l.id, d.hostname",
        )
        .bind(id)
        .fetch_all(db.pool())
        .await?
    } else {
        sqlx::query(
            "SELECT l.id, l.domain_id, d.hostname, l.slug, l.destination, l.redirect_type, l.enabled, \
             l.public_slug, l.public_enabled, l.public_threshold, l.public_sections, l.created_at, \
             CAST(COALESCE(SUM(s.clicks), 0) AS BIGINT) AS total_clicks \
             FROM insight_links l JOIN insight_domains d ON d.id = l.domain_id \
             LEFT JOIN insight_link_daily s ON s.link_id = l.id \
             GROUP BY l.id, d.hostname ORDER BY l.created_at DESC",
        )
        .fetch_all(db.pool())
        .await?
    };
    rows.iter().map(link_json).collect()
}

async fn fetch_link(db: &Database, id: &str) -> Result<Value, AppError> {
    link_rows(db, Some(id))
        .await?
        .into_iter()
        .next()
        .ok_or(AppError::NotFound)
}

fn link_json(row: &sqlx::any::AnyRow) -> Result<Value, AppError> {
    Ok(json!({
        "id": row.try_get::<String, _>("id")?,
        "domain_id": row.try_get::<String, _>("domain_id")?,
        "hostname": row.try_get::<String, _>("hostname")?,
        "slug": row.try_get::<String, _>("slug")?,
        "destination": row.try_get::<String, _>("destination")?,
        "redirect_type": row.try_get::<String, _>("redirect_type")?,
        "enabled": row.try_get::<i64, _>("enabled")? != 0,
        "public_slug": row.try_get::<String, _>("public_slug")?,
        "public_enabled": row.try_get::<i64, _>("public_enabled")? != 0,
        "public_threshold": row.try_get::<i64, _>("public_threshold")?,
        "public_sections": serde_json::from_str::<Vec<String>>(&row.try_get::<String, _>("public_sections")?).unwrap_or_default(),
        "created_at": row.try_get::<i64, _>("created_at")?,
        "total_clicks": row.try_get::<i64, _>("total_clicks")?,
    }))
}

async fn scalar(
    db: &Database,
    sql: &str,
    id: &str,
    since: i64,
    _unused: i64,
) -> Result<i64, AppError> {
    let _ = _unused;
    let row = sqlx::query(sql)
        .bind(id)
        .bind(since)
        .fetch_one(db.pool())
        .await?;
    Ok(row.try_get("n")?)
}

fn map_db(error: sqlx::Error) -> AppError {
    if error
        .as_database_error()
        .is_some_and(|db| db.is_unique_violation() || db.is_foreign_key_violation())
    {
        AppError::Conflict("That value is already in use, or it points at a missing record.".into())
    } else {
        AppError::from(error)
    }
}

fn field(name: &str, result: Result<String, &'static str>) -> Result<String, AppError> {
    result.map_err(|message| AppError::field(name, message))
}

fn public_token() -> String {
    let token = generate_token();
    let mut out = String::from("pub_");
    for character in token
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(20)
    {
        out.push(character);
    }
    out
}

fn client_ip(headers: &HeaderMap, proxy_hops: usize, peer: Option<IpAddr>) -> String {
    let forwarded = (proxy_hops > 0)
        .then(|| {
            headers
                .get("x-forwarded-for")?
                .to_str()
                .ok()?
                .split(',')
                .rev()
                .nth(proxy_hops - 1)
        })
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| clip(value, 64));
    forwarded
        .or_else(|| peer.map(|ip| ip.to_string()))
        .unwrap_or_else(|| "0.0.0.0".to_owned())
}

fn origin_host(origin: &str) -> Option<String> {
    let rest = origin.split_once("://")?.1;
    clean_host(rest.split('/').next()?).ok()
}

fn bots() -> &'static Bots {
    static BOTS: OnceLock<Bots> = OnceLock::new();
    BOTS.get_or_init(Bots::default)
}

fn with_cors(mut response: Response<Body>) -> Response<Body> {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, POST, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("content-type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    response
}
