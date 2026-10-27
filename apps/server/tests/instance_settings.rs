use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use statixo_platform::{SmtpConfig, TimestampMillis};
use statixo_server::{boot, initialize_setup_token, router, testing_config};
use tower::ServiceExt;

async fn started(env_smtp: bool) -> (Router, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("instance.sqlite");
    let _keep = Box::leak(Box::new(dir));
    let mut config = testing_config(path);
    if env_smtp {
        config.smtp = Some(SmtpConfig {
            host: "smtp.env.example".to_owned(),
            port: 2525,
            username: Some("env-user".to_owned()),
            password: Some("env-secret".to_owned()),
            from: "env@example.com".to_owned(),
        });
    }
    let state = boot(config).await.unwrap();
    let token = initialize_setup_token(&state.db, TimestampMillis::now())
        .await
        .unwrap()
        .expect("setup token");
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
    let pairs: Vec<&str> = response
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|value| value.split(';').next())
        .collect();
    let cookie = pairs
        .iter()
        .copied()
        .find(|value| value.starts_with("statixo_session="))
        .or_else(|| pairs.first().copied())
        .map(str::to_owned);
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
        "display_name": "Andras"
    })
}

async fn root_session(app: &Router, token: &str) -> String {
    let (status, user, cookie) = call(
        app,
        "POST",
        "/api/setup/complete",
        Some(owner_body(token)),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{user}");
    assert_eq!(user["root"], true);
    assert_eq!(user["installation_admin"], true);
    cookie.expect("session")
}

fn register_body(email: &str) -> Value {
    serde_json::json!({
        "email": email,
        "password": "correct-horse-battery",
        "display_name": "Member"
    })
}

#[tokio::test]
async fn root_opens_registration_only_after_smtp_and_members_cannot() {
    let (app, token) = started(false).await;
    let root = root_session(&app, &token).await;

    let (status, _, _) = call(&app, "GET", "/api/admin/settings", None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let (status, settings, _) = call(&app, "GET", "/api/admin/settings", None, Some(&root)).await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    assert_eq!(settings["registration_open"], false);
    assert_eq!(settings["smtp_ready"], false);
    assert_eq!(settings["initialized"], true);
    assert_eq!(settings["root_user"]["email"], "owner@coollabs.io");
    assert_eq!(settings["root_user"]["display_name"], "Andras");
    assert!(settings["smtp"].is_null());

    let (status, problem, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": true })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "email_not_configured");

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(register_body("second@coollabs.io")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");

    let (status, saved, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/smtp",
        Some(serde_json::json!({
            "host": "smtp.example.com",
            "port": 587,
            "username": "mailer",
            "password": "secret-pass",
            "from": "status@example.com"
        })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["smtp"]["source"], "saved");
    assert_eq!(saved["smtp"]["has_password"], true);
    assert_eq!(saved["smtp_ready"], true);
    assert!(!saved.to_string().contains("secret-pass"));

    let (status, opened, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": true })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    assert_eq!(opened["registration_open"], true);

    let (status, meta, _) = call(&app, "GET", "/api/meta", None, None).await;
    assert_eq!(meta["signup_open"], true);
    let _ = status;

    let (status, member, member_cookie) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(register_body("second@coollabs.io")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{member}");
    assert_eq!(member["role"], "member");
    assert_eq!(member["root"], false);
    assert_eq!(member["installation_admin"], false);
    let member_cookie = member_cookie.expect("member session");

    let (status, problem, _) = call(
        &app,
        "GET",
        "/api/admin/settings",
        None,
        Some(&member_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let (status, problem, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": false })),
        Some(&member_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    let (status, problem, _) =
        call(&app, "GET", "/api/admin/users", None, Some(&member_cookie)).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");

    let member_id = member["id"].as_str().unwrap();
    let (status, problem, _) = call(
        &app,
        "PUT",
        &format!("/api/admin/users/{member_id}/admin"),
        Some(serde_json::json!({ "admin": true })),
        Some(&member_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");

    let (status, promoted, _) = call(
        &app,
        "PUT",
        &format!("/api/admin/users/{member_id}/admin"),
        Some(serde_json::json!({ "admin": true })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{promoted}");
    assert_eq!(promoted["installation_admin"], true);
    assert_eq!(promoted["role"], "admin");
    assert_eq!(promoted["root"], false);

    let (status, me, _) = call(&app, "GET", "/api/auth/me", None, Some(&member_cookie)).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["installation_admin"], true);
    assert_eq!(me["root"], false);

    let (status, users, _) =
        call(&app, "GET", "/api/admin/users", None, Some(&member_cookie)).await;
    assert_eq!(status, StatusCode::OK, "{users}");
    assert!(
        users
            .as_array()
            .unwrap()
            .iter()
            .any(|user| user["root"] == true && user["email"] == "owner@coollabs.io")
    );

    let (status, problem, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": false })),
        Some(&member_cookie),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");

    let root_id = settings["root_user"]["id"].as_str().unwrap();
    let (status, problem, _) = call(
        &app,
        "PUT",
        &format!("/api/admin/users/{root_id}/admin"),
        Some(serde_json::json!({ "admin": false })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(
        problem["detail"],
        "The root user stays an installation admin."
    );

    let (status, demoted, _) = call(
        &app,
        "PUT",
        &format!("/api/admin/users/{member_id}/admin"),
        Some(serde_json::json!({ "admin": false })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{demoted}");
    assert_eq!(demoted["role"], "member");
    assert_eq!(demoted["installation_admin"], false);

    let (status, cleared, _) = call(
        &app,
        "DELETE",
        "/api/admin/settings/smtp",
        None,
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert_eq!(cleared["registration_open"], false);
    assert_eq!(cleared["smtp_ready"], false);
    assert!(cleared["smtp"].is_null());

    let (status, meta, _) = call(&app, "GET", "/api/meta", None, None).await;
    assert_eq!(meta["signup_open"], false);
    let _ = status;
    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(register_body("third@coollabs.io")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");

    let (status, problem, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": true })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{problem}");
    assert_eq!(problem["code"], "email_not_configured");
}

#[tokio::test]
async fn environment_smtp_can_open_registration_and_clearing_still_closes_it() {
    let (app, token) = started(true).await;
    let root = root_session(&app, &token).await;

    let (status, settings, _) = call(&app, "GET", "/api/admin/settings", None, Some(&root)).await;
    assert_eq!(status, StatusCode::OK, "{settings}");
    assert_eq!(settings["smtp_ready"], true);
    assert_eq!(settings["smtp"]["source"], "environment");
    assert_eq!(settings["smtp"]["host"], "smtp.env.example");
    assert!(!settings.to_string().contains("env-secret"));

    let (status, opened, _) = call(
        &app,
        "PUT",
        "/api/admin/settings/registration",
        Some(serde_json::json!({ "open": true })),
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{opened}");
    assert_eq!(opened["registration_open"], true);

    let (status, user, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(register_body("env@coollabs.io")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{user}");

    let (status, cleared, _) = call(
        &app,
        "DELETE",
        "/api/admin/settings/smtp",
        None,
        Some(&root),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{cleared}");
    assert_eq!(cleared["registration_open"], false);
    assert_eq!(cleared["smtp"]["source"], "environment");
    assert_eq!(cleared["smtp_ready"], true);

    let (status, problem, _) = call(
        &app,
        "POST",
        "/api/auth/register",
        Some(register_body("later@coollabs.io")),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{problem}");
    assert_eq!(problem["code"], "registration_closed");
}
