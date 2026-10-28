use std::io::Write;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use flate2::Compression;
use flate2::write::{DeflateEncoder, GzEncoder};
use glimwatch_platform::TimestampMillis;
use glimwatch_server::{boot, initialize_setup_token, permit_signup, router, testing_config};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

async fn app() -> (Router, Option<String>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("glimwatch.sqlite");
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
    body: Vec<u8>,
    cookie: Option<&str>,
    extra: &[(&str, &str)],
) -> (StatusCode, Value, Option<String>) {
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
    let allow = headers
        .get("access-control-allow-origin")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let cookie = session_cookie(&headers);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    let _ = allow;
    (status, json, cookie.or(allow))
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
        .find(|value| value.starts_with("glimwatch_session="))
        .or_else(|| pairs.first().copied())
        .map(str::to_owned)
}

fn envelope(event_json: &str) -> Vec<u8> {
    let header = format!(r#"{{"type":"event","length":{}}}"#, event_json.len());
    let mut body = b"{}\n".to_vec();
    body.extend_from_slice(header.as_bytes());
    body.push(b'\n');
    body.extend_from_slice(event_json.as_bytes());
    body.push(b'\n');
    body.extend_from_slice(br#"{"type":"session"}"#);
    body.push(b'\n');
    body.extend_from_slice(br#"{"sid":"s1","status":"ok"}"#);
    body.push(b'\n');
    body.extend_from_slice(br#"{"type":"client_report","length":2}"#);
    body.push(b'\n');
    body.extend_from_slice(br#"{}"#);
    body.push(b'\n');
    body
}

fn hex_id() -> String {
    glimwatch_platform::Id::new().to_string().replace('-', "")
}

fn sample() -> Value {
    json!({
        "event_id": "11111111111111111111111111111111",
        "level": "error",
        "platform": "php",
        "environment": "production",
        "release": "php@1",
        "sdk": {"name": "sentry.php", "version": "4.10.0"},
        "exception": {
            "values": [{
                "type": "RuntimeException",
                "value": "disk full on volume 3",
                "stacktrace": {
                    "frames": [{
                        "filename": "/app/src/Store.php",
                        "function": "save",
                        "lineno": 42,
                        "in_app": true,
                        "context_line": "throw new RuntimeException('disk full');",
                        "pre_context": ["$ok = true;"],
                        "post_context": ["}"]
                    }]
                }
            }]
        },
        "tags": {"runtime": "php"},
        "contexts": {"runtime": {"name": "php", "version": "8.3"}},
        "user": {"id": "9"},
        "request": {"method": "POST", "url": "http://app/pay"}
    })
}

async fn exercise(app: Router, setup_token: Option<String>) {
    let email = format!("errors-{}@coollabs.io", glimwatch_platform::Id::new());
    let (path, body) = if let Some(token) = setup_token {
        (
            "/api/setup/complete",
            json!({"token": token, "email": email, "password": "correct-horse-battery", "display_name": "Ada"}),
        )
    } else {
        (
            "/api/auth/register",
            json!({"email": email, "password": "correct-horse-battery", "display_name": "Ada"}),
        )
    };
    let (status, _, cookie) = call(
        &app,
        "POST",
        path,
        body.to_string().into_bytes(),
        None,
        &[("content-type", "application/json")],
    )
    .await;
    assert!(status.is_success(), "{status}");
    let cookie = cookie.unwrap();

    let (status, _, _) = call(&app, "GET", "/api/error-projects", Vec::new(), None, &[]).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, project, _) = call(
        &app,
        "POST",
        "/api/error-projects",
        json!({"name": "Checkout", "platform": "php"})
            .to_string()
            .into_bytes(),
        Some(&cookie),
        &[("content-type", "application/json")],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{project}");
    let id = project["id"].as_str().unwrap();
    let key = project["publicKey"].as_str().unwrap();
    assert!(project["dsn"].as_str().unwrap().contains(key));
    assert!(
        project["dsn"]
            .as_str()
            .unwrap()
            .ends_with(&format!("/{id}"))
    );

    let mut event = sample();
    let id1 = hex_id();
    let id2 = hex_id();
    let id3 = hex_id();
    let id4 = hex_id();
    event["event_id"] = json!(id1);
    let body = envelope(&event.to_string());
    let (status, _, _) = call(
        &app,
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
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut second = event.clone();
    second["event_id"] = json!(id2);
    second["exception"]["values"][0]["value"] = json!("disk full on volume 9");
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/{id}/store/"),
        second.to_string().into_bytes(),
        None,
        &[
            ("content-type", "application/json"),
            (
                "authorization",
                &format!("Sentry sentry_key={key}, sentry_version=7"),
            ),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut third = event.clone();
    third["event_id"] = json!(id3);
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(&envelope(&third.to_string())).unwrap();
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/{id}/envelope/"),
        gzip.finish().unwrap(),
        None,
        &[
            ("content-type", "application/x-sentry-envelope"),
            ("content-encoding", "gzip"),
            (
                "x-sentry-auth",
                &format!("Sentry sentry_key={key}, sentry_version=7"),
            ),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let mut fourth = event.clone();
    fourth["event_id"] = json!(id4);
    let mut deflate = DeflateEncoder::new(Vec::new(), Compression::default());
    deflate.write_all(fourth.to_string().as_bytes()).unwrap();
    let (status, _, _) = call(
        &app,
        "POST",
        &format!("/api/{id}/store/"),
        deflate.finish().unwrap(),
        None,
        &[
            ("content-type", "application/json"),
            ("content-encoding", "deflate"),
            (
                "x-sentry-auth",
                &format!("Sentry sentry_key={key}, sentry_version=7"),
            ),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, feed, _) = call(
        &app,
        "GET",
        &format!("/api/error-issues?project_id={id}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{feed}");
    assert_eq!(feed["issues"].as_array().unwrap().len(), 1, "{feed}");
    assert_eq!(feed["issues"][0]["eventCount"], 4);
    assert!(
        feed["issues"][0]["title"]
            .as_str()
            .unwrap()
            .contains("RuntimeException")
    );
    let slug = project["slug"].as_str().unwrap();
    let prefix: String = slug
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    assert!(
        feed["issues"][0]["shortId"]
            .as_str()
            .unwrap()
            .starts_with(&format!("{}-", prefix.to_uppercase())),
        "{feed}"
    );

    let (status, detail, _) = call(
        &app,
        "GET",
        &format!("/api/error-events/{id1}"),
        Vec::new(),
        Some(&cookie),
        &[],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    assert!(
        detail.to_string().contains("preContext") || detail.to_string().contains("pre_context")
    );

    let (status, _, allow) = call(
        &app,
        "OPTIONS",
        &format!("/api/{id}/envelope/"),
        Vec::new(),
        None,
        &[
            ("origin", "http://localhost:3000"),
            (
                "access-control-request-headers",
                "x-sentry-auth, content-type",
            ),
        ],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(allow.as_deref(), Some("*"));
}

#[tokio::test]
async fn sqlite_ingest_groups_events() {
    let (app, token) = app().await;
    exercise(app, token).await;
}

#[tokio::test]
async fn postgres_ingest_groups_events() {
    let Some(url) = std::env::var("GLIMWATCH_APP_POSTGRES_URL")
        .or_else(|_| std::env::var("STATUS_APP_POSTGRES_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres"))
    else {
        return;
    };
    let mut config = testing_config(std::path::PathBuf::from(
        "data/ignored-errors-postgres.sqlite",
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
    exercise(router(state), token).await;
}
