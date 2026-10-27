use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::response::Response;
use haruvo_errors::{decode_body, normalize_event, parse_envelope};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{Value, json};

use super::store;
use crate::error::AppError;
use crate::state::AppState;

const MAX_BODY: usize = 20 * 1024 * 1024;

#[derive(Debug, Deserialize, Default)]
pub struct SentryQuery {
    #[serde(default)]
    pub sentry_key: Option<String>,
    #[serde(default)]
    pub sentry_version: Option<String>,
    #[serde(default)]
    pub sentry_client: Option<String>,
}

pub async fn ingest_options(headers: HeaderMap) -> Response {
    cors_response(StatusCode::OK, HeaderMap::new(), None, &headers)
}

pub async fn ingest_envelope(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<SentryQuery>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let bytes = match read_body(request).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    let decoded = match decode_body(header_str(&headers, header::CONTENT_ENCODING), &bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            tracing::warn!(error = %error, "rejected compressed envelope");
            return ingest_error(&headers, StatusCode::BAD_REQUEST, error.to_string());
        }
    };
    let envelope = match parse_envelope(&decoded) {
        Ok(envelope) => envelope,
        Err(error) => {
            tracing::warn!(error = %error, "rejected envelope");
            return ingest_error(&headers, StatusCode::BAD_REQUEST, error.to_string());
        }
    };
    let dsn_key = envelope
        .header
        .get("dsn")
        .and_then(Value::as_str)
        .and_then(public_key_from_dsn);
    let key = match resolve_key(auth_key(&headers), query.sentry_key.clone(), dsn_key) {
        Ok(key) => key,
        Err(message) => return ingest_error(&headers, StatusCode::UNAUTHORIZED, message),
    };
    let project = match store::project_for_ingest(&state.db, &project_id, &key).await {
        Ok(project) => project,
        Err(error) => return ingest_error_from(&headers, error),
    };
    let mut last_id = None;
    for item in &envelope.items {
        match item.item_type.as_str() {
            "event" => match serde_json::from_slice::<Value>(&item.payload) {
                Ok(payload) => {
                    let event = normalize_event(payload);
                    match store::ingest_event(&state.db, &project, &event).await {
                        Ok(id) => last_id = Some(id),
                        Err(error) => {
                            tracing::error!(error = %error, "failed to store sentry event");
                            return ingest_error(
                                &headers,
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "failed to store event".into(),
                            );
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(error = %error, "skipped malformed event item");
                }
            },
            other => {
                tracing::debug!(
                    item_type = other,
                    bytes = item.payload.len(),
                    "ignored envelope item"
                );
            }
        }
    }
    let body = match last_id {
        Some(id) => json!({ "id": id }),
        None => json!({}),
    };
    json_ok(&headers, body)
}

pub async fn ingest_store(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<SentryQuery>,
    headers: HeaderMap,
    request: Request,
) -> Response {
    let bytes = match read_body(request).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    let decoded = match decode_body(header_str(&headers, header::CONTENT_ENCODING), &bytes) {
        Ok(decoded) => decoded,
        Err(error) => return ingest_error(&headers, StatusCode::BAD_REQUEST, error.to_string()),
    };
    let payload: Value = match serde_json::from_slice(&decoded) {
        Ok(payload) => payload,
        Err(_) => {
            return ingest_error(
                &headers,
                StatusCode::BAD_REQUEST,
                "store body is not JSON".into(),
            );
        }
    };
    let key = match resolve_key(auth_key(&headers), query.sentry_key, None) {
        Ok(key) => key,
        Err(message) => return ingest_error(&headers, StatusCode::UNAUTHORIZED, message),
    };
    let project = match store::project_for_ingest(&state.db, &project_id, &key).await {
        Ok(project) => project,
        Err(error) => return ingest_error_from(&headers, error),
    };
    let event = normalize_event(payload);
    match store::ingest_event(&state.db, &project, &event).await {
        Ok(id) => json_ok(&headers, json!({ "id": id })),
        Err(error) => {
            tracing::error!(error = %error, "failed to store sentry event");
            ingest_error(
                &headers,
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to store event".into(),
            )
        }
    }
}

pub async fn ingest_security(headers: HeaderMap) -> Response {
    json_ok(&headers, json!({}))
}

async fn read_body(request: Request) -> Result<Vec<u8>, Response> {
    let collected = request.into_body().collect().await;
    match collected {
        Ok(body) => {
            let bytes = body.to_bytes();
            if bytes.len() > MAX_BODY {
                Err(plain(StatusCode::PAYLOAD_TOO_LARGE, "payload too large"))
            } else {
                Ok(bytes.to_vec())
            }
        }
        Err(_) => Err(plain(StatusCode::PAYLOAD_TOO_LARGE, "payload too large")),
    }
}

fn auth_key(headers: &HeaderMap) -> Option<String> {
    header_str(headers, "x-sentry-auth")
        .and_then(sentry_key_from_header)
        .or_else(|| header_str(headers, header::AUTHORIZATION).and_then(sentry_key_from_header))
}

fn sentry_key_from_header(value: &str) -> Option<String> {
    let rest = value.trim().trim_start_matches("Sentry").trim();
    for part in rest.split(',') {
        let part = part.trim();
        let Some(raw) = part
            .strip_prefix("sentry_key=")
            .or_else(|| part.strip_prefix("sentry_key:"))
        else {
            continue;
        };
        let key = raw.trim().trim_matches('"').trim();
        if !key.is_empty() {
            return Some(key.to_owned());
        }
    }
    None
}

fn public_key_from_dsn(dsn: &str) -> Option<String> {
    let after_scheme = dsn.split("://").nth(1)?;
    let userinfo = after_scheme.split('@').next()?;
    let key = userinfo.split(':').next()?.trim();
    if key.is_empty() {
        None
    } else {
        Some(percent_decode(key))
    }
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                output.push(byte);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| value.to_owned())
}

fn resolve_key(
    header_key: Option<String>,
    query_key: Option<String>,
    dsn_key: Option<String>,
) -> Result<String, String> {
    let mut presented = Vec::new();
    if let Some(key) = header_key {
        presented.push(key);
    }
    if let Some(key) = query_key.filter(|key| !key.is_empty()) {
        presented.push(key);
    }
    if presented.is_empty() {
        return dsn_key.ok_or_else(|| "missing sentry public key".to_owned());
    }
    if presented.iter().skip(1).any(|key| key != &presented[0]) {
        return Err("sentry public key mismatch".into());
    }
    if let Some(dsn_key) = dsn_key {
        if dsn_key != presented[0] {
            return Err("dsn public key does not match sentry auth".into());
        }
    }
    Ok(presented.remove(0))
}

fn header_str<'a>(headers: &'a HeaderMap, name: impl header::AsHeaderName) -> Option<&'a str> {
    headers.get(name)?.to_str().ok()
}

fn json_ok(request_headers: &HeaderMap, body: Value) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    cors_response(
        StatusCode::OK,
        headers,
        Some(body.to_string().into_bytes()),
        request_headers,
    )
}

fn ingest_error(request_headers: &HeaderMap, status: StatusCode, message: String) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    if let Ok(value) = HeaderValue::from_str(&message) {
        headers.insert("x-sentry-error", value);
    }
    cors_response(
        status,
        headers,
        Some(json!({ "detail": message }).to_string().into_bytes()),
        request_headers,
    )
}

fn ingest_error_from(request_headers: &HeaderMap, error: AppError) -> Response {
    let (status, message) = match error {
        AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "invalid sentry public key".into()),
        AppError::NotFound => (StatusCode::NOT_FOUND, "unknown project".into()),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    };
    ingest_error(request_headers, status, message)
}

fn plain(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .body(Body::from(message.to_owned()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}

fn cors_response(
    status: StatusCode,
    mut headers: HeaderMap,
    body: Option<Vec<u8>>,
    request_headers: &HeaderMap,
) -> Response {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("POST, OPTIONS"),
    );
    let allow = request_headers
        .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
        .and_then(|value| value.to_str().ok())
        .unwrap_or(
            "Content-Type, X-Sentry-Auth, Authorization, X-Requested-With, baggage, sentry-trace",
        );
    if let Ok(value) = HeaderValue::from_str(allow) {
        headers.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, value);
    }
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    let mut response = Response::builder().status(status);
    for (name, value) in headers.iter() {
        response = response.header(name, value);
    }
    response
        .body(Body::from(body.unwrap_or_default()))
        .unwrap_or_else(|_| Response::new(Body::empty()))
}
