use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use owlee_platform::TimestampMillis;
use owlee_server::{
    SetupError, boot, initialize_setup_token, rotate_setup_token, router, testing_config,
};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

async fn started(open_signup: bool) -> (Router, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("setup.sqlite");
    let _keep = Box::leak(Box::new(dir));
    let mut config = testing_config(path);
    config.open_signup = open_signup;
    let state = boot(config).await.unwrap();
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .expect("fresh database issues one setup token");
    (router(state), token.token)
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

fn owner_body(token: &str) -> Value {
    serde_json::json!({
        "token": token,
        "email": "owner@coollabs.io",
        "password": "correct-horse-battery",
        "display_name": "Owner"
    })
}

#[tokio::test]
async fn setup_succeeds_once_and_public_signup_stays_closed() {
    let (app, token) = started(false).await;

    let (status, meta, _) = call(&app, "GET", "/api/meta", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(meta["signup_open"], false);
    assert_eq!(meta["setup_complete"], false);
    assert!(meta["demo"].is_null());

    let (status, body, _) = call(&app, "GET", "/api/setup/status", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["complete"], false);

    let (status, problem, _) = call(
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
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");

    let (status, problem, cookie) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body("not-the-token")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
    assert_eq!(problem["code"], "invalid_setup_token");
    assert!(cookie.is_none());

    let (status, user, cookie) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    assert_eq!(user["role"], "owner");
    assert_eq!(user["root"], true);
    assert_eq!(user["installation_admin"], true);
    assert_eq!(user["email"], "owner@coollabs.io");
    let cookie = cookie.expect("session cookie");

    let (status, me, _) = call(&app, "GET", "/api/auth/me", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["email"], "owner@coollabs.io");
    assert_eq!(me["root"], true);

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "setup_unavailable");

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "second@coollabs.io",
            "password": "correct-horse-battery",
            "display_name": "Second"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");

    let (status, meta, _) = call(&app, "GET", "/api/meta", None, None).await;
    assert_eq!(meta["setup_complete"], true);
    assert_eq!(meta["signup_open"], false);
    let _ = status;
}

#[tokio::test]
async fn env_open_signup_does_not_open_registration() {
    let (app, token) = started(true).await;
    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "ada@coollabs.io",
            "password": "correct-horse-battery"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");

    let (status, _, _) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(serde_json::json!({
            "email": "second@coollabs.io",
            "password": "correct-horse-battery",
            "display_name": "Second"
        })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");
    let (status, meta, _) = call(&app, "GET", "/api/meta", None, None).await;
    assert_eq!(status, StatusCode::OK, "{meta}");
    assert_eq!(meta["signup_open"], false);
}

#[tokio::test]
async fn rotate_issues_a_new_unused_token_and_refuses_after_setup() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("rotate.sqlite");
    let _keep = Box::leak(Box::new(dir));
    let state = boot(testing_config(path)).await.unwrap();
    let now = TimestampMillis::now();
    let first = initialize_setup_token(&state.db, now)
        .await
        .unwrap()
        .unwrap();
    let repeated = initialize_setup_token(&state.db, now).await.unwrap();
    assert!(repeated.is_none(), "a live token is not printed again");

    let rotated = rotate_setup_token(&state.db, now).await.unwrap();
    assert_ne!(rotated.token, first.token);
    let db = state.db.clone();
    let app = router(state);

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&first.token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{problem}");
    assert_eq!(problem["code"], "invalid_setup_token");

    let (status, user, cookie) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&rotated.token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    assert!(cookie.is_some());
    assert_eq!(user["role"], "owner");

    let error = rotate_setup_token(&db, now).await.unwrap_err();
    assert!(matches!(error, SetupError::AlreadyComplete));
}

#[tokio::test]
async fn legacy_session_cookies_still_authenticate() {
    let (app, token) = started(false).await;
    let (status, _, cookie) = call(
        &app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(&token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let cookie = cookie.expect("session cookie");
    assert!(cookie.starts_with("owlee_session="), "{cookie}");
    let value = cookie.split_once('=').unwrap().1;
    for name in ["glimwatch_session", "status_session"] {
        let (status, me, _) = call(
            &app,
            "GET",
            "/api/auth/me",
            None,
            Some(&format!("{name}={value}")),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{name} {me}");
    }
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
