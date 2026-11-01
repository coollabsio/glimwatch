use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use owlee_platform::TimestampMillis;
use owlee_server::{boot, initialize_setup_token, permit_signup, router, testing_config};
use serde_json::{Value, json};
use tower::ServiceExt;

const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 Chrome/128.0.0.0 Safari/537.36";

async fn app() -> (Router, Option<String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("owlee.sqlite");
    let _keep = Box::leak(Box::new(dir));
    let state = boot(testing_config(path)).await.unwrap();
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .map(|issued| issued.token);
    (router(state), token)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>, Option<String>) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let response = app
        .clone()
        .oneshot(
            request
                .body(Body::from(
                    body.map(|value| value.to_string()).unwrap_or_default(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let cookie = session_cookie(&headers);
    let location = headers
        .get("location")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json, cookie, location)
}

fn session_cookie(headers: &axum::http::HeaderMap) -> Option<String> {
    let pairs: Vec<&str> = headers
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .collect();
    pairs
        .iter()
        .copied()
        .find(|value| value.starts_with("owlee_session="))
        .or_else(|| pairs.first().copied())
        .map(str::to_owned)
}

async fn owner(app: &Router, token: &str) -> String {
    let (status, body, cookie, _) = call(
        app,
        "POST",
        "/api/setup/complete",
        Some(json!({
            "token": token,
            "email": "ada@coollabs.io",
            "password": "correct-horse-battery",
            "display_name": "Ada"
        })),
        None,
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    cookie.unwrap()
}

fn hex_id() -> String {
    owlee_platform::Id::new().to_string().replace('-', "")
}

#[tokio::test]
async fn dashboard_aggregates_monitors_errors_and_traffic() {
    let (app, token) = app().await;
    let (status, _, _, _) = call(&app, "GET", "/api/dashboard", None, None, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let cookie = owner(&app, token.as_deref().expect("setup token")).await;
    let (status, empty, _, _) = call(&app, "GET", "/api/dashboard", None, Some(&cookie), &[]).await;
    assert_eq!(status, StatusCode::OK, "{empty}");
    assert_eq!(empty["monitors"]["up"], 0);
    assert_eq!(empty["monitors"]["down"], 0);
    assert_eq!(empty["monitors"]["degraded"], 0);
    assert_eq!(empty["monitors"]["pending"], 0);
    assert_eq!(empty["monitors"]["paused"], 0);
    assert!(empty["monitors"]["uptime_today"].is_null());
    assert!(empty["monitors"]["failing"].as_array().unwrap().is_empty());
    assert_eq!(empty["incidents"]["open"], 0);
    assert!(empty["pages"].as_array().unwrap().is_empty());
    assert_eq!(empty["errors"]["projects"], 0);
    assert_eq!(empty["errors"]["unresolved"], 0);
    assert_eq!(empty["errors"]["new_24h"], 0);
    assert_eq!(empty["errors"]["events_24h"].as_array().unwrap().len(), 24);
    assert!(
        empty["errors"]["events_24h"]
            .as_array()
            .unwrap()
            .iter()
            .all(|value| value.as_i64() == Some(0))
    );
    assert!(empty["errors"]["issues"].as_array().unwrap().is_empty());
    assert_eq!(empty["analytics"]["sites"], 0);
    assert_eq!(empty["analytics"]["links"], 0);
    assert_eq!(empty["analytics"]["days"], 7);
    assert_eq!(empty["analytics"]["pageviews"], 0);
    assert_eq!(empty["analytics"]["visitors"], 0);
    assert_eq!(empty["analytics"]["clicks"], 0);
    assert_eq!(empty["analytics"]["previous_pageviews"], 0);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/health", axum::routing::get(|| async { "ok" })),
        )
        .await
        .unwrap();
    });

    let (status, monitor, _, _) = call(
        &app,
        "POST",
        "/api/monitors",
        Some(json!({
            "name": "API",
            "job_type": "http",
            "url": format!("http://{address}/health"),
            "interval_seconds": 30,
            "timeout_ms": 3000,
            "failure_threshold": 1
        })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{monitor}");
    let api_id = monitor["id"].as_str().unwrap();
    let (status, checked, _, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{api_id}/checks"),
        None,
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    assert_eq!(checked["state"], "up");

    let (status, down, _, _) = call(
        &app,
        "POST",
        "/api/monitors",
        Some(json!({
            "name": "Worker",
            "job_type": "http",
            "url": "http://127.0.0.1:9/health",
            "timeout_ms": 1000,
            "interval_seconds": 30,
            "failure_threshold": 1
        })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{down}");
    let worker_id = down["id"].as_str().unwrap();
    let (status, checked, _, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{worker_id}/checks"),
        None,
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    assert_eq!(checked["state"], "down");

    let (status, page, _, _) = call(
        &app,
        "POST",
        "/api/pages",
        Some(json!({
            "title": "coolLabs",
            "slug": "coollabs",
            "published": true,
            "monitor_ids": [api_id, worker_id]
        })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{page}");

    let (status, project, _, _) = call(
        &app,
        "POST",
        "/api/error-projects",
        Some(json!({"name": "Checkout", "platform": "php"})),
        Some(&cookie),
        &[],
    )
    .await;
    assert!(status.is_success(), "{status} {project}");
    let project_id = project["id"].as_str().unwrap();
    let key = project["publicKey"].as_str().unwrap();
    let event_id = hex_id();
    let (status, stored, _, _) = call(
        &app,
        "POST",
        &format!("/api/{project_id}/store/"),
        Some(json!({
            "event_id": event_id,
            "level": "error",
            "platform": "php",
            "exception": {
                "values": [{
                    "type": "RuntimeException",
                    "value": "disk full on volume 3"
                }]
            }
        })),
        None,
        &[(
            "authorization",
            &format!("Sentry sentry_key={key}, sentry_version=7"),
        )],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{stored}");

    let (status, site, _, _) = call(
        &app,
        "POST",
        "/api/sites",
        Some(json!({"name": "Docs", "domain": "coolify.io"})),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let (status, _, _, _) = call(
        &app,
        "POST",
        "/api/receive",
        Some(json!({
            "site": site["public_id"],
            "type": "pageview",
            "url": "https://coolify.io/docs",
            "title": "Docs"
        })),
        None,
        &[
            ("user-agent", BROWSER),
            ("origin", "https://coolify.io"),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let (status, domain, _, _) = call(
        &app,
        "POST",
        "/api/domains",
        Some(json!({"hostname": "go.owlee.test", "enabled": true})),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{domain}");
    let (status, link, _, _) = call(
        &app,
        "POST",
        "/api/links",
        Some(json!({
            "domain_id": domain["id"],
            "slug": "coolify",
            "destination": "https://coolify.io",
            "redirect_type": "temporary"
        })),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let (status, _, _, location) = call(
        &app,
        "GET",
        "/go/coolify",
        None,
        None,
        &[("user-agent", BROWSER)],
    )
    .await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(location.as_deref(), Some("https://coolify.io"));

    let (status, body, _, _) = call(&app, "GET", "/api/dashboard", None, Some(&cookie), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["monitors"]["up"], 1);
    assert_eq!(body["monitors"]["down"], 1);
    assert_eq!(body["monitors"]["degraded"], 0);
    let uptime = body["monitors"]["uptime_today"].as_f64().unwrap();
    assert!((uptime - 0.5).abs() < 0.01, "{uptime}");
    assert!(body["monitors"]["avg_latency_ms"].as_i64().unwrap() >= 0);
    let failing = body["monitors"]["failing"].as_array().unwrap();
    assert_eq!(failing.len(), 1, "{body}");
    assert_eq!(failing[0]["name"], "Worker");
    assert_eq!(failing[0]["state"], "down");
    assert!(failing[0].get("bars").is_none());
    assert_eq!(failing[0]["uptime_today"].as_f64().unwrap(), 0.0);
    assert_eq!(body["incidents"]["open"], 1);
    let incident = &body["incidents"]["items"][0];
    assert_eq!(incident["title"], "Worker is down");
    assert_eq!(incident["status"], "investigating");
    assert_eq!(failing[0]["since"], incident["started_at"]);
    assert_eq!(body["pages"].as_array().unwrap().len(), 1);
    assert_eq!(body["pages"][0]["title"], "coolLabs");
    assert_eq!(body["pages"][0]["banner"], "partial_outage");
    assert_eq!(body["pages"][0]["published"], true);
    assert_eq!(body["errors"]["projects"], 1);
    assert!(body["errors"]["unresolved"].as_i64().unwrap() >= 1);
    assert!(body["errors"]["new_24h"].as_i64().unwrap() >= 1);
    assert_eq!(body["errors"]["events_24h"].as_array().unwrap().len(), 24);
    let events: i64 = body["errors"]["events_24h"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_i64().unwrap())
        .sum();
    assert!(events >= 1, "{body}");
    assert!(body["errors"]["issues"][0]["event_count"].as_i64().unwrap() >= 1);
    assert!(
        body["errors"]["issues"][0]["title"]
            .as_str()
            .unwrap()
            .contains("disk full")
    );
    assert_eq!(body["analytics"]["sites"], 1);
    assert_eq!(body["analytics"]["links"], 1);
    assert!(body["analytics"]["pageviews"].as_i64().unwrap() >= 1);
    assert!(body["analytics"]["visitors"].as_i64().unwrap() >= 1);
    assert_eq!(body["analytics"]["previous_pageviews"], 0);
    assert!(body["analytics"]["clicks"].as_i64().unwrap() >= 1);
    assert_eq!(body["analytics"]["previous_clicks"], 0);
    assert_eq!(body["analytics"]["top_sites"][0]["name"], "Docs");
    assert!(
        body["analytics"]["top_sites"][0]["pageviews"]
            .as_i64()
            .unwrap()
            >= 1
    );
}

#[tokio::test]
async fn postgres_dashboard_query_runs_when_configured() {
    let Some(url) = std::env::var("OWLEE_APP_POSTGRES_URL")
        .ok()
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let mut config = testing_config(std::path::PathBuf::from(
        "data/ignored-dashboard-postgres.sqlite",
    ));
    config.database_url = Some(url);
    let state = boot(config).await.expect("postgres boot");
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .map(|issued| issued.token);
    if token.is_none() {
        permit_signup(&state.db).await.unwrap();
    }
    let app = router(state);
    let email = format!("dash-{}@coollabs.io", owlee_platform::Id::new());
    let (status, user, cookie, _) = if let Some(token) = token {
        call(
            &app,
            "POST",
            "/api/setup/complete",
            Some(json!({
                "token": token,
                "email": email,
                "password": "correct-horse-battery",
                "display_name": "Postgres"
            })),
            None,
            &[],
        )
        .await
    } else {
        call(
            &app,
            "POST",
            "/api/auth/register",
            Some(json!({
                "email": email,
                "password": "correct-horse-battery",
                "display_name": "Postgres"
            })),
            None,
            &[],
        )
        .await
    };
    assert!(status.is_success(), "{status} {user}");
    let (status, body, _, _) =
        call(&app, "GET", "/api/dashboard", None, cookie.as_deref(), &[]).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["analytics"]["days"], 7);
    assert_eq!(body["errors"]["events_24h"].as_array().unwrap().len(), 24);
    assert!(body["monitors"]["failing"].is_array());
    assert!(body["pages"].is_array());
}
