//! Demo error project and a few grouped events for `haruvo seed`.
use haruvo_errors::normalize_event;
use serde_json::{Value, json};

use super::store;
use crate::error::AppError;
use crate::state::AppState;

pub async fn seed_demo(state: &AppState) -> Result<(), AppError> {
    let project = store::create_project(&state.db, "Checkout", "php").await?;
    let base = sample(&project.public_key, &project.id);
    for (id, message) in [
        ("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "disk full on volume 3"),
        ("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", "disk full on volume 9"),
        ("cccccccccccccccccccccccccccccccc", "disk full on volume 1"),
    ] {
        let mut event = base.clone();
        event["event_id"] = json!(id);
        event["exception"]["values"][0]["value"] = json!(message);
        store::ingest_event(&state.db, &project, &normalize_event(event)).await?;
    }
    let mut rust = json!({
        "event_id": "dddddddddddddddddddddddddddddddd",
        "level": "error",
        "platform": "rust",
        "environment": "production",
        "release": "api@1.4.0",
        "exception": {
            "values": [{
                "type": "ExampleError",
                "value": "payment provider timed out",
                "mechanism": {"type": "generic", "handled": false},
                "stacktrace": {
                    "frames": [{
                        "filename": "src/pay.rs",
                        "function": "checkout::charge",
                        "lineno": 88,
                        "in_app": true,
                        "context_line": "return Err(ExampleError(\"payment provider timed out\"));"
                    }]
                }
            }]
        },
        "tags": {"service": "payments"},
        "user": {"id": "42", "email": "ada@example.com"}
    });
    rust["event_id"] = json!("dddddddddddddddddddddddddddddddd");
    let rust_project = store::create_project(&state.db, "API", "rust").await?;
    store::ingest_event(&state.db, &rust_project, &normalize_event(rust)).await?;
    let _ = project;
    Ok(())
}

fn sample(key: &str, project_id: &str) -> Value {
    json!({
        "event_id": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "level": "error",
        "platform": "php",
        "environment": "production",
        "release": "checkout@2.4.0",
        "sdk": {"name": "sentry.php", "version": "4.10.0"},
        "exception": {
            "values": [{
                "type": "RuntimeException",
                "value": "disk full on volume 3",
                "mechanism": {"type": "generic", "handled": false},
                "stacktrace": {
                    "frames": [
                        {
                            "filename": "/app/src/Http.php",
                            "function": "dispatch",
                            "lineno": 18,
                            "in_app": false
                        },
                        {
                            "filename": "/app/src/Store.php",
                            "function": "save",
                            "lineno": 42,
                            "in_app": true,
                            "context_line": "throw new RuntimeException('disk full');",
                            "pre_context": ["$ok = true;"],
                            "post_context": ["}"]
                        }
                    ]
                }
            }]
        },
        "breadcrumbs": {"values": [
            {"category": "query", "message": "insert into orders", "level": "info", "timestamp": "2026-10-21T08:00:00.000Z"},
            {"category": "http", "message": "POST /pay", "level": "info"}
        ]},
        "tags": {"runtime": "php", "region": "eu"},
        "contexts": {"runtime": {"name": "php", "version": "8.3"}},
        "user": {"id": "9", "email": "buyer@example.com"},
        "request": {"method": "POST", "url": "https://shop.example/pay"},
        "extra": {"dsn_hint": format!("http://{key}@127.0.0.1/{project_id}")}
    })
}
