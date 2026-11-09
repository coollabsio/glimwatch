//! HTTP checks for the public ingest guards.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use owlee_platform::{Config, EnvironmentMode, TimestampMillis};
use owlee_server::{
    AppState, MapResolver, boot, initialize_setup_token, permit_signup, prune_before,
    quota_today, recheck_verified, router, testing_config,
};
use serde_json::{Value, json};
use sqlx::Row;
use tower::ServiceExt;

const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 Chrome/128.0.0.0 Safari/537.36";

struct Hit {
    status: StatusCode,
    body: Value,
    retry_after: Option<String>,
    rate_limits: Option<String>,
    cookie: Option<String>,
}

async fn boot_app(mut config: Config) -> (Router, AppState, Option<String>) {
    if config.database_url.is_none() {
        let dir = tempfile::tempdir().unwrap();
        config.database_path = dir.path().join("owlee.sqlite");
        let _keep = Box::leak(Box::new(dir));
    }
    let state = boot(config).await.expect("boot");
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .map(|issued| issued.token);
    if token.is_none() {
        permit_signup(&state.db).await.unwrap();
    }
    let app = router(state.clone());
    (app, state, token)
}

fn site_host() -> String {
    let raw = owlee_platform::Id::new().to_string().replace('-', "");
    format!("d{}.example", &raw[..12])
}

fn sqlite_config() -> Config {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owlee.sqlite");
    let _keep = Box::leak(Box::new(dir));
    testing_config(path)
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    body: Vec<u8>,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
) -> Hit {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let retry_after = headers
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let rate_limits = headers
        .get("x-sentry-rate-limits")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let cookie = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .find_map(|value| value.split(';').next())
        .filter(|value| value.starts_with("owlee_session="))
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Hit {
        status,
        body,
        retry_after,
        rate_limits,
        cookie,
    }
}

async fn owner(app: &Router, setup_token: Option<&str>) -> String {
    let (path, body) = if let Some(token) = setup_token {
        (
            "/api/setup/complete",
            json!({
                "token": token,
                "email": format!("guard-{}@coollabs.io", owlee_platform::Id::new()),
                "password": "correct-horse-battery",
                "display_name": "Ada"
            }),
        )
    } else {
        (
            "/api/auth/register",
            json!({
                "email": format!("guard-{}@coollabs.io", owlee_platform::Id::new()),
                "password": "correct-horse-battery",
                "display_name": "Ada"
            }),
        )
    };
    let hit = send(
        app,
        "POST",
        path,
        body.to_string().into_bytes(),
        None,
        &[("content-type", "application/json")],
    )
    .await;
    assert!(hit.status.is_success(), "{} {}", hit.status, hit.body);
    hit.cookie.expect("session")
}

fn envelope(event_json: &str) -> Vec<u8> {
    let header = format!(r#"{{"type":"event","length":{}}}"#, event_json.len());
    let mut body = b"{}\n".to_vec();
    body.extend_from_slice(header.as_bytes());
    body.push(b'\n');
    body.extend_from_slice(event_json.as_bytes());
    body
}

fn event_id() -> String {
    owlee_platform::Id::new().to_string().replace('-', "")
}

fn event(id: &str, message: &str) -> Value {
    json!({
        "event_id": id,
        "level": "error",
        "platform": "javascript",
        "exception": {
            "values": [{
                "type": "Error",
                "value": message,
                "stacktrace": {"frames": [{"filename": "app.js", "function": "main", "in_app": true}]}
            }]
        }
    })
}

async fn project(app: &Router, cookie: &str) -> (String, String) {
    let hit = send(
        app,
        "POST",
        "/api/error-projects",
        json!({"name": "Checkout", "platform": "javascript"})
            .to_string()
            .into_bytes(),
        Some(cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(hit.status, StatusCode::OK, "{}", hit.body);
    (
        hit.body["id"].as_str().unwrap().to_owned(),
        hit.body["publicKey"].as_str().unwrap().to_owned(),
    )
}

async fn post_event(app: &Router, id: &str, key: &str, body: Vec<u8>) -> Hit {
    send(
        app,
        "POST",
        &format!("/api/{id}/envelope/?sentry_key={key}"),
        body,
        None,
        &[
            ("content-type", "application/x-sentry-envelope"),
            (
                "x-sentry-auth",
                &format!("Sentry sentry_key={key}, sentry_version=7"),
            ),
        ],
    )
    .await
}

async fn exercise(config: Config) {
    oversized_body(&config).await;
    burst_is_429(config.clone()).await;
    wrong_origin(config.clone()).await;
    flood_is_capped(config.clone()).await;
    retention_keeps_rollups(config.clone()).await;
    quota_is_enforced(config.clone()).await;
    override_replaces_the_global_cap(config.clone()).await;
    txt_verification(config.clone()).await;
    login_is_limited(config).await;
}

async fn oversized_body(config: &Config) {
    let (app, _, token) = boot_app(config.clone()).await;
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    let mut huge = "x".repeat(config.ingest.error_max_body_bytes + 8);
    huge.insert_str(0, "{");
    let hit = post_event(&app, &id, &key, huge.into_bytes()).await;
    assert_eq!(hit.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", hit.body);

    let (app, _, token) = boot_app(config.clone()).await;
    let cookie = owner(&app, token.as_deref()).await;
    let created = send(
        &app,
        "POST",
        "/api/sites",
        json!({"name": "Docs", "domain": site_host()})
            .to_string()
            .into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let public_id = created.body["public_id"].as_str().unwrap();
    let fat = "y".repeat(config.ingest.analytics_max_body_bytes + 8);
    let body = format!(r#"{{"site":"{public_id}","type":"pageview","url":"https://docs.example/","pad":"{fat}"}}"#);
    let rejected = send(
        &app,
        "POST",
        "/api/receive",
        body.into_bytes(),
        None,
        &[
            ("content-type", "application/json"),
            ("origin", "https://docs.example"),
            ("user-agent", BROWSER),
        ],
    )
    .await;
    assert_eq!(rejected.status, StatusCode::PAYLOAD_TOO_LARGE, "{}", rejected.body);
}

async fn burst_is_429(mut config: Config) {
    config.ingest.error_rate_ip_burst = 2;
    config.ingest.error_rate_ip_per_min = 2;
    config.ingest.error_rate_project_burst = 100;
    let (app, _, token) = boot_app(config).await;
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    for index in 0..2 {
        let body = envelope(&event(&event_id(), "disk full").to_string());
        let hit = post_event(&app, &id, &key, body).await;
        assert_eq!(hit.status, StatusCode::OK, "{}", hit.body);
    }
    let blocked = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "disk full").to_string()),
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS, "{}", blocked.body);
    let retry = blocked.retry_after.as_deref().unwrap_or("");
    assert!(!retry.is_empty() && retry != "0", "{retry}");
    assert!(
        blocked
            .rate_limits
            .as_deref()
            .unwrap_or("")
            .contains(":error:"),
        "{:?}",
        blocked.rate_limits
    );
}

async fn wrong_origin(config: Config) {
    let (app, state, token) = boot_app(config).await;
    let cookie = owner(&app, token.as_deref()).await;
    let created = send(
        &app,
        "POST",
        "/api/sites",
        json!({"name": "Docs", "domain": site_host()})
            .to_string()
            .into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    let public_id = created.body["public_id"].as_str().unwrap().to_owned();
    let site_id = created.body["id"].as_str().unwrap().to_owned();
    let domain = created.body["domain"].as_str().unwrap().to_owned();
    let payload = json!({
        "site": public_id,
        "type": "pageview",
        "url": "https://evil.example/"
    })
    .to_string()
    .into_bytes();
    let rejected = send(
        &app,
        "POST",
        "/api/receive",
        payload.clone(),
        None,
        &[
            ("content-type", "application/json"),
            ("origin", "https://evil.example"),
            ("user-agent", BROWSER),
        ],
    )
    .await;
    assert_eq!(rejected.status, StatusCode::FORBIDDEN, "{}", rejected.body);
    let accepted = send(
        &app,
        "POST",
        "/api/receive",
        json!({"site": public_id, "type": "pageview", "url": format!("https://{domain}/guide")})
            .to_string()
            .into_bytes(),
        None,
        &[
            ("content-type", "application/json"),
            ("origin", &format!("https://{domain}")),
            ("user-agent", BROWSER),
        ],
    )
    .await;
    assert_eq!(accepted.status, StatusCode::ACCEPTED, "{}", accepted.body);
    let rows = sqlx::query("SELECT COUNT(*) AS n FROM insight_events WHERE site_id = $1")
        .bind(&site_id)
        .fetch_one(state.db.pool())
        .await
        .unwrap();
    let count: i64 = rows.try_get("n").unwrap();
    assert_eq!(count, 1, "the rejected origin was not stored");
}

async fn flood_is_capped(config: Config) {
    let keep = config.ingest.error_keep_first;
    let (app, _, token) = boot_app(config).await;
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    for _ in 0..8 {
        let body = envelope(
            &event(&event_id(), "disk full on volume 3").to_string(),
        );
        let hit = post_event(&app, &id, &key, body).await;
        assert_eq!(hit.status, StatusCode::OK, "{}", hit.body);
    }
    let feed = send(
        &app,
        "GET",
        &format!("/api/error-issues?project_id={id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(feed.status, StatusCode::OK, "{}", feed.body);
    assert_eq!(feed.body["issues"].as_array().unwrap().len(), 1);
    assert_eq!(feed.body["issues"][0]["eventCount"], 8);
    let issue_id = feed.body["issues"][0]["id"].as_str().unwrap();
    let detail = send(
        &app,
        "GET",
        &format!("/api/error-issues/{issue_id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    let stored = detail.body["events"].as_array().unwrap().len() as i64;
    assert!(stored <= keep, "stored {stored} payloads, cap is {keep}");
    assert!(stored >= 1);
}

async fn retention_keeps_rollups(config: Config) {
    let (app, state, token) = boot_app(config).await;
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    let first = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "old failure").to_string()),
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    let second = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "old failure").to_string()),
    )
    .await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.body);
    let created = send(
        &app,
        "POST",
        "/api/sites",
        json!({"name": "Docs", "domain": site_host()})
            .to_string()
            .into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    let public_id = created.body["public_id"].as_str().unwrap();
    let site_id = created.body["id"].as_str().unwrap().to_owned();
    let domain = created.body["domain"].as_str().unwrap().to_owned();
    let viewed = send(
        &app,
        "POST",
        "/api/receive",
        json!({"site": public_id, "type": "pageview", "url": format!("https://{domain}/")})
            .to_string()
            .into_bytes(),
        None,
        &[
            ("content-type", "application/json"),
            ("origin", &format!("https://{domain}")),
            ("user-agent", BROWSER),
        ],
    )
    .await;
    assert_eq!(viewed.status, StatusCode::ACCEPTED, "{}", viewed.body);
    let old_id = first.body["id"].as_str().unwrap();
    sqlx::query("UPDATE events SET timestamp = '2000-01-01T00:00:00.000Z' WHERE id = $1")
        .bind(old_id)
        .execute(state.db.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE insight_events SET timestamp = 10 WHERE site_id = $1")
        .bind(&site_id)
        .execute(state.db.pool())
        .await
        .unwrap();
    let stats = prune_before(&state.db, "2020-01-01T00:00:00.000Z", 1_000_000)
        .await
        .unwrap();
    assert!(stats.error_events >= 1);
    assert!(stats.analytics_events >= 1);
    let feed = send(
        &app,
        "GET",
        &format!("/api/error-issues?project_id={id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(feed.body["issues"][0]["eventCount"], 2);
    let issue_id = feed.body["issues"][0]["id"].as_str().unwrap();
    let detail = send(
        &app,
        "GET",
        &format!("/api/error-issues/{issue_id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(detail.body["events"].as_array().unwrap().len(), 1);
    let overview = send(
        &app,
        "GET",
        &format!("/api/sites/{site_id}/overview"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(overview.status, StatusCode::OK, "{}", overview.body);
    assert_eq!(overview.body["pageviews"], 1);
    let left = sqlx::query("SELECT COUNT(*) AS n FROM insight_events WHERE site_id = $1")
        .bind(&site_id)
        .fetch_one(state.db.pool())
        .await
        .unwrap();
    assert_eq!(left.try_get::<i64, _>("n").unwrap(), 0);
}

async fn quota_is_enforced(config: Config) {
    let (app, state, token) = boot_app(config).await;
    let root = token.is_some();
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    if root {
        let before = send(
            &app,
            "GET",
            "/api/admin/settings",
            Vec::new(),
            Some(&cookie),
            &[],
        )
        .await;
        assert_eq!(before.status, StatusCode::OK, "{}", before.body);
        let error_quota = before.body["error_daily_quota"].as_i64().unwrap();
        let analytics_quota = before.body["analytics_daily_quota"].as_i64().unwrap();
        let updated = send(
            &app,
            "PUT",
            "/api/admin/settings/quotas",
            json!({"error_daily": error_quota, "analytics_daily": analytics_quota + 1})
                .to_string()
                .into_bytes(),
            Some(&cookie),
            &[("content-type", "application/json")],
        )
        .await;
        assert_eq!(updated.status, StatusCode::OK, "{}", updated.body);
        assert_eq!(updated.body["analytics_daily_quota"], analytics_quota + 1);
        let restored = send(
            &app,
            "PUT",
            "/api/admin/settings/quotas",
            json!({"error_daily": error_quota, "analytics_daily": analytics_quota})
                .to_string()
                .into_bytes(),
            Some(&cookie),
            &[("content-type", "application/json")],
        )
        .await;
        assert_eq!(restored.status, StatusCode::OK, "{}", restored.body);
    }
    let current = send(
        &app,
        "GET",
        &format!("/api/error-projects/{id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(current.status, StatusCode::OK, "{}", current.body);
    let error_quota = current.body["dailyQuota"].as_i64().unwrap();
    sqlx::query(
        "INSERT INTO ingest_usage (kind, subject_id, day, count) VALUES ('error', $1, $2, $3) \
         ON CONFLICT (kind, subject_id, day) DO UPDATE SET count = $3",
    )
    .bind(&id)
    .bind(quota_today())
    .bind(error_quota)
    .execute(state.db.pool())
    .await
    .unwrap();
    let blocked = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "over quota").to_string()),
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS, "{}", blocked.body);
    assert!(blocked.retry_after.is_some());
    assert!(
        blocked
            .rate_limits
            .as_deref()
            .unwrap_or("")
            .contains("quota_exceeded"),
        "{:?}",
        blocked.rate_limits
    );
    let listed = send(
        &app,
        "GET",
        "/api/error-projects",
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    let project = listed
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == id)
        .unwrap();
    assert_eq!(project["quotaExceeded"], true);
    sqlx::query("DELETE FROM ingest_usage WHERE subject_id = $1")
        .bind(&id)
        .execute(state.db.pool())
        .await
        .unwrap();
}

async fn override_replaces_the_global_cap(config: Config) {
    let (app, _, token) = boot_app(config).await;
    let cookie = owner(&app, token.as_deref()).await;
    let (id, key) = project(&app, &cookie).await;
    let first = send(
        &app,
        "GET",
        &format!("/api/error-projects/{id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.body);
    assert!(first.body["quotaOverride"].is_null());
    let inherited = first.body["dailyQuota"].as_i64().unwrap();
    assert!(inherited > 1, "global cap {inherited} should be above the override");
    assert!(!first.body["resetsAt"].as_str().unwrap_or("").is_empty());

    let set = send(
        &app,
        "PUT",
        &format!("/api/error-projects/{id}/quota"),
        json!({"daily_quota": 1}).to_string().into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(set.status, StatusCode::OK, "{}", set.body);
    assert_eq!(set.body["dailyQuota"], 1);
    assert_eq!(set.body["quotaOverride"], 1);

    let accepted = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "over the project cap").to_string()),
    )
    .await;
    assert_eq!(accepted.status, StatusCode::OK, "{}", accepted.body);
    let blocked = post_event(
        &app,
        &id,
        &key,
        envelope(&event(&event_id(), "over the project cap").to_string()),
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS, "{}", blocked.body);
    assert!(
        blocked.rate_limits.as_deref().unwrap_or("").contains("quota_exceeded"),
        "{:?}",
        blocked.rate_limits
    );

    let home = send(&app, "GET", "/api/dashboard", Vec::new(), Some(&cookie), &[]).await;
    assert_eq!(home.status, StatusCode::OK, "{}", home.body);
    let hits = home.body["quota_hits"].as_array().unwrap();
    assert!(
        hits.iter().any(|hit| hit["kind"] == "error" && hit["id"] == id),
        "{hits:?}"
    );
    assert!(!hits[0]["resets_at"].as_str().unwrap_or("").is_empty());

    let cleared = send(
        &app,
        "PUT",
        &format!("/api/error-projects/{id}/quota"),
        json!({"daily_quota": null}).to_string().into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(cleared.status, StatusCode::OK, "{}", cleared.body);
    assert!(cleared.body["quotaOverride"].is_null());
    assert_eq!(cleared.body["dailyQuota"], inherited);

    let domain = site_host();
    let created = send(
        &app,
        "POST",
        "/api/sites",
        json!({"name": "Docs", "domain": domain.clone()}).to_string().into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    assert!(created.body["quota_override"].is_null());
    let site_inherited = created.body["daily_quota"].as_i64().unwrap();
    assert!(site_inherited > 1);
    let site_id = created.body["id"].as_str().unwrap().to_owned();
    let public_id = created.body["public_id"].as_str().unwrap().to_owned();
    let set_site = send(
        &app,
        "PUT",
        &format!("/api/sites/{site_id}/quota"),
        json!({"daily_quota": 1}).to_string().into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(set_site.status, StatusCode::OK, "{}", set_site.body);
    assert_eq!(set_site.body["daily_quota"], 1);
    assert_eq!(set_site.body["quota_override"], 1);
    let origin = format!("https://{domain}");
    let first_view = receive(&app, &public_id, &origin).await;
    assert_eq!(first_view.status, StatusCode::ACCEPTED, "{}", first_view.body);
    let second_view = receive(&app, &public_id, &origin).await;
    assert_eq!(second_view.status, StatusCode::TOO_MANY_REQUESTS, "{}", second_view.body);
    assert!(second_view.retry_after.is_some());
    let again = send(&app, "GET", "/api/dashboard", Vec::new(), Some(&cookie), &[]).await;
    let site_hits = again.body["quota_hits"].as_array().unwrap();
    assert!(
        site_hits.iter().any(|hit| hit["kind"] == "analytics" && hit["id"] == site_id),
        "{site_hits:?}"
    );
}

async fn txt_verification(mut config: Config) {
    config.environment = EnvironmentMode::Production;
    config.ingest.allow_localhost = false;
    let mut state = boot(config).await.unwrap();
    let dns = MapResolver::default();
    state.dns = Arc::new(dns.clone());
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .map(|issued| issued.token);
    if token.is_none() {
        permit_signup(&state.db).await.unwrap();
    }
    let app = router(state.clone());
    let cookie = owner(&app, token.as_deref()).await;
    let domain = site_host();
    let created = send(
        &app,
        "POST",
        "/api/sites",
        json!({"name": "Example", "domain": domain.clone()})
            .to_string()
            .into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    assert_eq!(created.body["verified"], false);
    assert_eq!(created.body["ingest_open"], false);
    let site_id = created.body["id"].as_str().unwrap();
    let public_id = created.body["public_id"].as_str().unwrap();
    let name = created.body["verify_name"].as_str().unwrap().to_owned();
    let value = created.body["verify_value"].as_str().unwrap().to_owned();
    let origin = format!("https://{domain}");
    assert_eq!(name, format!("_owlee-verify.{domain}"));
    assert!(value.starts_with("owlee-verify="));
    let early = receive(&app, public_id, &origin).await;
    assert_eq!(early.status, StatusCode::FORBIDDEN, "{}", early.body);

    dns.set(&name, vec!["unrelated".into()]);
    let failed = send(
        &app,
        "POST",
        &format!("/api/sites/{site_id}/verify"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_REQUEST, "{}", failed.body);
    let still = get_site(&app, site_id, &cookie).await;
    assert_eq!(still["verified"], false);

    dns.set(&name, vec![value.clone()]);
    let ok = send(
        &app,
        "POST",
        &format!("/api/sites/{site_id}/verify"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.body);
    assert_eq!(ok.body["verified"], true);
    let accepted = receive(&app, public_id, &origin).await;
    assert_eq!(accepted.status, StatusCode::ACCEPTED, "{}", accepted.body);

    dns.set(&name, Vec::new());
    let cleared = recheck_verified(&state).await.unwrap();
    assert_eq!(cleared, 1);
    let after = get_site(&app, site_id, &cookie).await;
    assert_eq!(after["verified"], false);
    let again = receive(&app, public_id, &origin).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN, "{}", again.body);
}

async fn receive(app: &Router, public_id: &str, origin: &str) -> Hit {
    send(
        app,
        "POST",
        "/api/receive",
        json!({"site": public_id, "type": "pageview", "url": format!("{origin}/")})
            .to_string()
            .into_bytes(),
        None,
        &[
            ("content-type", "application/json"),
            ("origin", origin),
            ("user-agent", BROWSER),
        ],
    )
    .await
}

async fn get_site(app: &Router, id: &str, cookie: &str) -> Value {
    let hit = send(
        app,
        "GET",
        &format!("/api/sites/{id}"),
        Vec::new(),
        Some(cookie),
        &[],
    )
    .await;
    assert_eq!(hit.status, StatusCode::OK, "{}", hit.body);
    hit.body
}

async fn login_is_limited(mut config: Config) {
    config.proxy_hops = 1;
    config.ingest.login_rate_ip_burst = 2;
    config.ingest.login_rate_ip_per_min = 2;
    config.ingest.login_rate_account_burst = 100;
    config.ingest.login_rate_account_per_min = 100;
    let (app, _, token) = boot_app(config).await;
    let _cookie = owner(&app, token.as_deref()).await;
    let body = json!({"email": "nobody@coollabs.io", "password": "wrong-password-1"})
        .to_string()
        .into_bytes();
    for index in 0..2 {
        let attempt = send(
            &app,
            "POST",
            "/api/auth/login",
            body.clone(),
            None,
            &[
                ("content-type", "application/json"),
                ("x-forwarded-for", "203.0.113.10"),
            ],
        )
        .await;
        assert_eq!(attempt.status, StatusCode::UNAUTHORIZED, "{index}");
    }
    let blocked = send(
        &app,
        "POST",
        "/api/auth/login",
        body.clone(),
        None,
        &[
            ("content-type", "application/json"),
            ("x-forwarded-for", "203.0.113.10"),
        ],
    )
    .await;
    assert_eq!(blocked.status, StatusCode::TOO_MANY_REQUESTS, "{}", blocked.body);
    assert!(blocked.retry_after.is_some());
    let other_ip = send(
        &app,
        "POST",
        "/api/auth/login",
        body,
        None,
        &[
            ("content-type", "application/json"),
            ("x-forwarded-for", "203.0.113.11"),
        ],
    )
    .await;
    assert_eq!(
        other_ip.status,
        StatusCode::UNAUTHORIZED,
        "a different forwarded address has its own bucket"
    );
}

#[tokio::test]
async fn sqlite_ingest_guards() {
    exercise(sqlite_config()).await;
}

#[tokio::test]
async fn postgres_ingest_guards() {
    let Some(url) = std::env::var("OWLEE_APP_POSTGRES_URL")
        .ok()
        .filter(|url| url.starts_with("postgres"))
    else {
        return;
    };
    let mut config = sqlite_config();
    // Own database so this test's owner is root and it does not share
    // instance_settings with the other Postgres suites.
    let url = url.trim_end_matches('/').rsplit_once('/').map_or(url.clone(), |(base, _)| {
        format!("{base}/owlee_ingest")
    });
    let admin = url.replacen("owlee_ingest", "postgres", 1);
    let _ = sqlx::query("CREATE DATABASE owlee_ingest")
        .execute(&sqlx::postgres::PgPoolOptions::new().connect(&admin).await.unwrap())
        .await;
    config.database_url = Some(url);
    exercise(config).await;
}
