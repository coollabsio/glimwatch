use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use owlee_platform::TimestampMillis;
use owlee_server::{boot, initialize_setup_token, permit_signup, router, testing_config};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;

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
    user_agent: Option<&str>,
) -> (StatusCode, Value, Option<String>, Option<String>) {
    let mut request = Request::builder().method(method).uri(uri);
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if let Some(user_agent) = user_agent {
        request = request.header("user-agent", user_agent);
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
        serde_json::from_slice(&bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
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

const BROWSER: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_5) AppleWebKit/537.36 Chrome/128.0.0.0 Safari/537.36";
const BOT: &str = "Mozilla/5.0 (compatible; Googlebot/2.1; +http://www.google.com/bot.html)";

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
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    cookie.unwrap()
}

#[tokio::test]
async fn sqlite_collects_pageviews_and_counts_short_link_clicks() {
    let (app, token) = app().await;
    let cookie = owner(&app, token.as_deref().expect("setup token")).await;
    let (status, script, _, _) = call(&app, "GET", "/data.js", None, None, Some(BROWSER)).await;
    assert_eq!(status, StatusCode::OK);
    let script = script.as_str().unwrap_or("");
    assert!(script.contains("/api/receive"), "{script}");
    assert!(script.contains("window.owleeEvent"), "{script}");

    let (status, site, _, _) = call(
        &app,
        "POST",
        "/api/sites",
        Some(json!({"name": "Docs", "domain": "coolify.io"})),
        Some(&cookie),
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{site}");
    let site_id = site["id"].as_str().unwrap();
    let public_id = site["public_id"].as_str().unwrap();

    let (status, missing, _, _) = call(
        &app,
        "POST",
        "/api/receive",
        Some(json!({"site": "nope", "type": "pageview", "url": "https://coolify.io/"})),
        None,
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{missing}");

    let (status, _, _, _) = call(
        &app,
        "POST",
        "/api/receive",
        Some(json!({
            "site": public_id,
            "type": "pageview",
            "url": "https://coolify.io/docs?utm_source=github",
            "referrer": "https://github.com/coollabsio/coolify",
            "title": "Docs",
            "hostname": "coolify.io"
        })),
        None,
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let (status, _, _, _) = call(
        &app,
        "POST",
        "/api/receive",
        Some(json!({"site": public_id, "type": "pageview", "url": "https://coolify.io/"})),
        None,
        Some(BOT),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let (status, overview, _, _) = call(
        &app,
        "GET",
        &format!("/api/sites/{site_id}/overview"),
        None,
        Some(&cookie),
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["pageviews"], 1, "bots are accepted and not stored");
    assert!(overview["series"].as_array().unwrap().len() > 1);

    let (status, pages, _, _) = call(
        &app,
        "GET",
        &format!("/api/sites/{site_id}/breakdown/path"),
        None,
        Some(&cookie),
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pages}");
    assert_eq!(pages["rows"][0]["label"], "/docs", "{pages}");

    let (status, domain, _, _) = call(
        &app,
        "POST",
        "/api/domains",
        Some(json!({"hostname": "go.owlee.test", "enabled": true})),
        Some(&cookie),
        Some(BROWSER),
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
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{link}");
    let (status, _, _, location) =
        call(&app, "GET", "/go/coolify", None, None, Some(BROWSER)).await;
    assert_eq!(status, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(location.as_deref(), Some("https://coolify.io"));
    let (status, clicks, _, _) = call(
        &app,
        "GET",
        &format!("/api/links/{}/overview", link["id"].as_str().unwrap()),
        None,
        Some(&cookie),
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{clicks}");
    assert_eq!(clicks["clicks"], 1);
}

#[tokio::test]
async fn postgres_collects_a_pageview_when_configured() {
    let Some(url) = std::env::var("OWLEE_APP_POSTGRES_URL")
        .ok()
        .filter(|url| !url.is_empty())
    else {
        return;
    };
    let mut config = testing_config(std::path::PathBuf::from(
        "data/ignored-insights-postgres.sqlite",
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
    let email = format!("insights-{}@coollabs.io", owlee_platform::Id::new());
    let (status, _user, cookie, _) = if let Some(token) = token {
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
            Some(BROWSER),
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
            Some(BROWSER),
        )
        .await
    };
    assert!(status.is_success(), "{status}");
    let cookie = cookie.unwrap();
    let (status, site, _, _) = call(
        &app,
        "POST",
        "/api/sites",
        Some(json!({"name": "PG", "domain": "postgres.owlee.test"})),
        Some(&cookie),
        Some(BROWSER),
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
            "url": "https://postgres.owlee.test/ready"
        })),
        None,
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, overview, _, _) = call(
        &app,
        "GET",
        &format!("/api/sites/{}/overview", site["id"].as_str().unwrap()),
        None,
        Some(&cookie),
        Some(BROWSER),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{overview}");
    assert_eq!(overview["pageviews"], 1);
}
