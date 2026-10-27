use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use haruvo_platform::TimestampMillis;
use haruvo_server::{
    DEMO_PASSWORD, boot, initialize_setup_token, permit_signup, router, testing_config,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn app() -> (Router, Option<String>) {
    app_with_targets(Vec::new()).await
}

async fn app_with_targets(connectivity_targets: Vec<String>) -> (Router, Option<String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("status.sqlite");
    // Leak the temp dir so the sqlite file outlives the router.
    let _keep = Box::leak(Box::new(dir));
    let mut config = testing_config(path);
    config.connectivity_targets = connectivity_targets;
    let state = boot(config).await.unwrap();
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .map(|issued| issued.token);
    (router(state), token)
}

async fn owner(app: &Router, token: &str) -> (Value, String) {
    let (status, user, cookie) = call(
        app,
        "POST",
        "/api/setup/complete",
        Some(serde_json::json!({
            "token": token,
            "email": "ada@coollabs.io",
            "password": "correct-horse-battery",
            "display_name": "Ada"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    assert_eq!(user["role"], "owner");
    (user, cookie.expect("session cookie"))
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
    let cookie = session_cookie(response.headers());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json, cookie)
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
        .find(|value| value.starts_with("haruvo_session="))
        .or_else(|| pairs.first().copied())
        .map(str::to_owned)
}

#[tokio::test]
async fn sqlite_flow_covers_checks_incidents_and_the_public_page() {
    let (app, token) = app().await;
    let (status, health, _) = call(&app, "GET", "/api/health", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(health["status"], "ok");

    let (status, _, _) = call(&app, "GET", "/api/auth/me", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (user, cookie) = owner(&app, token.as_deref().expect("setup token")).await;
    assert_eq!(user["email"], "ada@coollabs.io");

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

    let starts = haruvo_platform::TimestampMillis::now();
    let ends = haruvo_platform::TimestampMillis::from_millis(starts.as_millis() + 3_600_000);
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
    let Some(url) = std::env::var("HARUVO_APP_POSTGRES_URL")
        .or_else(|_| std::env::var("STATUS_APP_POSTGRES_URL"))
        .ok()
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let mut config = testing_config(std::path::PathBuf::from("data/ignored-postgres.sqlite"));
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
    let email = format!("pg-{}@coollabs.io", haruvo_platform::Id::new());
    let (status, user, _) = if let Some(token) = token {
        call(
            &app,
            "POST",
            "/api/setup/complete",
            Some(serde_json::json!({
                "token": token,
                "email": email,
                "password": "correct-horse-battery",
                "display_name": "Postgres"
            })),
            None,
        )
        .await
    } else {
        call(
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
        .await
    };
    assert!(status.is_success(), "{status} {user}");
    let (status, ready, _) = call(&app, "GET", "/api/health/ready", None, None).await;
    assert_eq!(status, StatusCode::OK, "{ready}");
    assert_eq!(ready["database"], "postgres");
}

#[tokio::test]
async fn sign_up_rejects_the_seeded_demo_password() {
    let (app, token) = app().await;
    let (status, body, cookie) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(serde_json::json!({
            "token": token.expect("setup token"),
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
    let (app, token) = app_with_targets(vec!["127.0.0.1:9".to_owned()]).await;
    let (_, cookie) = owner(&app, token.as_deref().expect("setup token")).await;
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

async fn agent_call(app: &Router, uri: &str, body: Value, token: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn agent_registers_its_own_checks_and_reports_outcomes() {
    let (app, token) = app().await;
    let (_, cookie) = owner(&app, token.as_deref().expect("setup token")).await;
    let (status, agent, _) = call(
        &app,
        "POST",
        "/api/agents",
        Some(serde_json::json!({ "name": "vpc-eu" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{agent}");
    let token = agent["token"].as_str().unwrap().to_owned();
    let agent_id = agent["id"].as_str().unwrap().to_owned();
    assert_eq!(agent["online"], false);

    let checks = |names: &[&str]| {
        serde_json::json!({
            "checks": names.iter().map(|name| serde_json::json!({
                "name": name,
                "check_type": "postgres",
                "target": "10.0.0.5:5432",
                "interval_seconds": 30,
                "timeout_ms": 10000,
                "failure_threshold": 1,
            })).collect::<Vec<_>>()
        })
    };
    let (status, _) = agent_call(&app, "/api/agent/checks", checks(&["Main DB"]), "wrong").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, registered) = agent_call(
        &app,
        "/api/agent/checks",
        checks(&["Main DB", "Replica"]),
        &token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{registered}");
    let id = registered["checks"][0]["monitor_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let replica = registered["checks"][1]["monitor_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (_, again) = agent_call(&app, "/api/agent/checks", checks(&["Main DB"]), &token).await;
    assert_eq!(
        again["checks"][0]["monitor_id"],
        id.as_str(),
        "same name, same monitor"
    );
    let (_, monitors, _) = call(&app, "GET", "/api/monitors", None, Some(&cookie)).await;
    let monitors = monitors.as_array().unwrap();
    assert_eq!(monitors.len(), 2, "{monitors:?}");
    let main = monitors
        .iter()
        .find(|monitor| monitor["id"] == id.as_str())
        .unwrap();
    assert_eq!(main["check_type"], "postgres");
    assert_eq!(main["agent_id"], agent_id.as_str());
    let dropped = monitors
        .iter()
        .find(|monitor| monitor["id"] == replica.as_str())
        .unwrap();
    assert_eq!(
        dropped["active"], false,
        "a check removed from the config is paused"
    );

    let outcome = serde_json::json!({
        "monitor_id": id,
        "status": "failure",
        "status_code": null,
        "latency_ms": 12,
        "error": "password authentication failed"
    });
    let (status, body) = agent_call(&app, "/api/agent/results", outcome.clone(), &token).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
    let (_, monitor, _) = call(
        &app,
        "GET",
        &format!("/api/monitors/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(monitor["state"], "down", "{monitor}");
    assert_eq!(
        monitor["recent"][0]["error"],
        "password authentication failed"
    );
    assert_eq!(monitor["recent"][0]["region"], "vpc-eu");
    let (_, incidents, _) = call(&app, "GET", "/api/incidents", None, Some(&cookie)).await;
    assert_eq!(incidents.as_array().unwrap().len(), 1, "{incidents}");

    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/monitors/{id}/checks"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the server does not run agent checks"
    );
    let (status, _, _) = call(
        &app,
        "PUT",
        &format!("/api/monitors/{id}"),
        Some(serde_json::json!({ "name": "x", "job_type": "tcp", "url": "evil.internal:22" })),
        Some(&cookie),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the dashboard cannot change an agent target"
    );

    let (_, other, _) = call(
        &app,
        "POST",
        "/api/agents",
        Some(serde_json::json!({ "name": "vpc-us" })),
        Some(&cookie),
    )
    .await;
    let (status, _) = agent_call(
        &app,
        "/api/agent/results",
        outcome,
        other["token"].as_str().unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let (_, agents, _) = call(&app, "GET", "/api/agents", None, Some(&cookie)).await;
    let first = &agents.as_array().unwrap()[0];
    assert_eq!(first["name"], "vpc-eu", "{agents}");
    assert_eq!(first["online"], true);
    assert_eq!(first["monitor_count"], 2);
    assert!(first.get("token").is_none());

    let (status, problem, _) = call(
        &app,
        "DELETE",
        &format!("/api/agents/{agent_id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
}
