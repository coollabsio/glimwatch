use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use status_server::{DEMO_PASSWORD, boot, router, testing_config};
use tower::ServiceExt;

async fn app() -> Router {
    app_with_targets(Vec::new()).await
}

async fn app_with_targets(connectivity_targets: Vec<String>) -> Router {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("status.sqlite");
    // Leak the temp dir so the sqlite file outlives the router.
    let _keep = Box::leak(Box::new(dir));
    let mut config = testing_config(path);
    config.connectivity_targets = connectivity_targets;
    let state = boot(config).await.unwrap();
    router(state)
}

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    cookie: Option<&str>,
) -> (StatusCode, Value, Option<String>) {
    let request = Request::builder().method(method).uri(uri);
    let request = if body.is_some() {
        request.header("content-type", "application/json")
    } else {
        request
    };
    let request = if let Some(cookie) = cookie {
        request.header("cookie", cookie)
    } else {
        request
    };
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
    let cookie = response
        .headers()
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json, cookie)
}

#[tokio::test]
async fn sqlite_flow_covers_checks_incidents_and_the_public_page() {
    let app = app().await;
    let (status, health, _) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");

    let (status, _, _) = call(&app, "GET", "/api/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, user, cookie) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "ada@coollabs.io",
            "password": "correct-horse-battery",
            "display_name": "Ada"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    assert_eq!(user["role"], "owner");
    let cookie = cookie.expect("session cookie");

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

    let (status, monitor, _) = call(
        &app,
        "POST",
        "/api/monitors",
        Some(serde_json::json!({
            "name": "API",
            "job_type": "http",
            "url": format!("http://{address}/health"),
            "interval_seconds": 30,
            "timeout_ms": 3000,
            "failure_threshold": 2,
            "assertions": [{ "type": "body", "compare": "contains", "target": "ok" }]
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{monitor}");
    let api_id = monitor["id"].as_str().unwrap().to_owned();

    let (status, checked, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{api_id}/checks"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    assert_eq!(checked["state"], "up");
    assert!(checked["uptime_24h"].as_f64().unwrap() > 0.99);
    assert_eq!(checked["recent"][0]["status"], "success");

    let (status, down, _) = call(
        &app,
        "POST",
        "/api/monitors",
        Some(serde_json::json!({
            "name": "Worker",
            "job_type": "http",
            "url": "http://127.0.0.1:9/health",
            "timeout_ms": 1500,
            "interval_seconds": 30,
            "failure_threshold": 1
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{down}");
    let worker_id = down["id"].as_str().unwrap().to_owned();
    let (status, checked, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{worker_id}/checks"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    assert_eq!(checked["state"], "down");

    let (status, incidents, _) = call(&app, "GET", "/api/incidents", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK, "{incidents}");
    assert_eq!(incidents.as_array().unwrap().len(), 1);
    assert_eq!(incidents[0]["status"], "investigating");
    assert_eq!(incidents[0]["auto"], true);
    let incident_id = incidents[0]["id"].as_str().unwrap();

    let (status, updated, _) = call(
        &app,
        "POST",
        &format!("/api/incidents/{incident_id}/updates"),
        Some(serde_json::json!({
            "status": "identified",
            "message": "The worker process is gone."
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{updated}");
    assert_eq!(updated["updates"].as_array().unwrap().len(), 2);

    let (status, page, _) = call(
        &app,
        "POST",
        "/api/pages",
        Some(serde_json::json!({
            "title": "coolLabs",
            "slug": "coollabs",
            "description": "Platform status",
            "published": true,
            "monitor_ids": [api_id, worker_id]
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{page}");

    let (status, public, _) = call(&app, "GET", "/api/public/coollabs", None, None).await;
    assert_eq!(status, StatusCode::OK, "{public}");
    assert_eq!(public["banner"], "partial_outage");
    assert_eq!(public["components"].as_array().unwrap().len(), 2);
    assert!(!public["incidents"].as_array().unwrap().is_empty());

    let starts = status_platform::TimestampMillis::now();
    let ends = status_platform::TimestampMillis::from_millis(starts.as_millis() + 3_600_000);
    let (status, maintenance, _) = call(
        &app,
        "POST",
        "/api/maintenances",
        Some(serde_json::json!({
            "title": "API restart",
            "message": "Rolling the API.",
            "page_id": page["id"],
            "monitor_ids": [api_id],
            "starts_at": starts,
            "ends_at": ends
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{maintenance}");
    let (status, checked, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{api_id}/checks"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{checked}");
    assert_eq!(checked["recent"][0]["status"], "maintenance");
    assert_eq!(checked["state"], "up");

    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1);
    let hook = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let hook_addr = hook.local_addr().unwrap();
    tokio::spawn(async move {
        let app = Router::new().route(
            "/hook",
            axum::routing::post(move |body: String| {
                let tx = tx.clone();
                async move {
                    let _ = tx.send(body).await;
                    StatusCode::NO_CONTENT
                }
            }),
        );
        axum::serve(hook, app).await.unwrap();
    });
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(hook_addr).await.is_ok() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (status, channel, _) = call(
        &app,
        "POST",
        "/api/notifications",
        Some(serde_json::json!({
            "name": "Hooks",
            "provider": "webhook",
            "config": { "url": format!("http://{hook_addr}/hook"), "secret": "topsecret" },
            "monitor_ids": []
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{channel}");
    let channel_id = channel["id"].as_str().unwrap();
    let (status, tested, _) = call(
        &app,
        "POST",
        &format!("/api/notifications/{channel_id}/test"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tested}");
    let body = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(body.contains("notification.test"), "{body}");
    assert_eq!(tested["deliveries"][0]["status"], "sent");

    let (status, _, _) = call(&app, "POST", "/api/auth/logout", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = call(&app, "GET", "/api/auth/me", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn postgres_accepts_the_same_api() {
    let Some(url) = std::env::var("STATUS_APP_POSTGRES_URL")
        .ok()
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let mut config = testing_config(std::path::PathBuf::from("data/ignored-postgres.sqlite"));
    config.database_url = Some(url);
    let state = boot(config).await.expect("postgres boot");
    let app = router(state);
    let email = format!("pg-{}@coollabs.io", status_platform::Id::new());
    let (status, user, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": email,
            "password": "correct-horse-battery",
            "display_name": "Postgres"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{user}");
    let (status, ready, _) = call(&app, "GET", "/api/health/ready", None, None).await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["database"], "postgres");
}

#[tokio::test]
async fn sign_up_rejects_the_seeded_demo_password() {
    let app = app().await;
    let (status, body, cookie) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "ada@coollabs.io",
            "password": DEMO_PASSWORD,
        })),
        None,
    )
    .await;
    assert!(status.is_client_error(), "{status} {body}");
    assert!(cookie.is_none());
}

#[tokio::test]
async fn failed_check_is_not_counted_when_the_host_has_no_network() {
    let app = app_with_targets(vec!["127.0.0.1:9".to_owned()]).await;
    let (_, _, cookie) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "ada@coollabs.io",
            "password": "correct-horse-battery",
        })),
        None,
    )
    .await;
    let cookie = cookie.expect("session cookie");
    let (status, monitor, _) = call(
        &app,
        "POST",
        "/api/monitors",
        Some(serde_json::json!({
            "name": "Worker",
            "job_type": "tcp",
            "url": "127.0.0.1:9",
            "timeout_ms": 1000,
            "interval_seconds": 30,
            "failure_threshold": 1
        })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{monitor}");
    let id = monitor["id"].as_str().unwrap();

    let (status, problem, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{id}/checks"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{problem}");
    assert_eq!(problem["code"], "host_offline");

    let (_, monitor, _) = call(
        &app,
        "GET",
        &format!("/api/monitors/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(monitor["state"], "pending", "{monitor}");
    let (_, incidents, _) = call(&app, "GET", "/api/incidents", None, Some(&cookie)).await;
    assert_eq!(incidents.as_array().unwrap().len(), 0, "{incidents}");
}
