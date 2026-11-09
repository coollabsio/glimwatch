use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{self, HeaderMap, HeaderValue};
use axum::response::Response;
use http_body_util::BodyExt;
use owlee_errors::{decode_body, normalize_event, parse_envelope};
use serde::Deserialize;
use serde_json::{Value, json};

use super::store;
use crate::error::AppError;
use crate::peer::{self, client_ip};
use crate::quota;
use crate::state::AppState;

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
    let max_body = state.config.ingest.error_max_body_bytes;
    let peer = peer_of(&request);
    let bytes = match read_limited(request, max_body, &headers).await {
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
    if decoded.len() > max_body {
        return too_large(&headers);
    }
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
    if let Some(response) = enforce_rate(&state, &headers, peer, &project.id) {
        return response;
    }
    let mut last_id = None;
    for item in &envelope.items {
        match item.item_type.as_str() {
            "event" => match serde_json::from_slice::<Value>(&item.payload) {
                Ok(payload) => {
                    let event = normalize_event(payload);
                    match accept_event(&state, &headers, &project, &event).await {
                        Ok(Accepted::Stored(id)) => last_id = Some(id),
                        Ok(Accepted::Limited(response)) => return response,
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
    let max_body = state.config.ingest.error_max_body_bytes;
    let peer = peer_of(&request);
    let bytes = match read_limited(request, max_body, &headers).await {
        Ok(bytes) => bytes,
        Err(response) => return response,
    };
    let decoded = match decode_body(header_str(&headers, header::CONTENT_ENCODING), &bytes) {
        Ok(decoded) => decoded,
        Err(error) => return ingest_error(&headers, StatusCode::BAD_REQUEST, error.to_string()),
    };
    if decoded.len() > max_body {
        return too_large(&headers);
    }
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
    if let Some(response) = enforce_rate(&state, &headers, peer, &project.id) {
        return response;
    }
    let event = normalize_event(payload);
    match accept_event(&state, &headers, &project, &event).await {
        Ok(Accepted::Stored(id)) => json_ok(&headers, json!({ "id": id })),
        Ok(Accepted::Limited(response)) => response,
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

enum Accepted {
    Stored(String),
    Limited(Response),
}

fn peer_of(request: &Request) -> Option<std::net::IpAddr> {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
        .or_else(|| peer::peer_addr(request.extensions()))
}

fn enforce_rate(
    state: &AppState,
    headers: &HeaderMap,
    peer: Option<std::net::IpAddr>,
    project_id: &str,
) -> Option<Response> {
    let ip = client_ip(headers, state.config.proxy_hops, peer);
    let ip_limit = state.error_ip_limit.check(&format!("error-ip:{ip}"));
    if !ip_limit.allowed {
        return Some(rate_limited(headers, ip_limit.retry_after_secs, "ip", ""));
    }
    let project_limit = state
        .error_project_limit
        .check(&format!("error-project:{project_id}"));
    if !project_limit.allowed {
        return Some(rate_limited(
            headers,
            project_limit.retry_after_secs,
            "key",
            "",
        ));
    }
    None
}

async fn accept_event(
    state: &AppState,
    headers: &HeaderMap,
    project: &store::Project,
    event: &owlee_errors::NormalizedEvent,
) -> Result<Accepted, String> {
    let global = quota::error_quota(&state.db)
        .await
        .map_err(|error| error.to_string())?;
    let quota = quota::effective(global, project.daily_quota);
    let ingested = store::ingest_event(
        &state.db,
        project,
        event,
        quota,
        state.config.ingest.error_keep_first,
        state.config.ingest.error_sample_window_secs,
    )
    .await
    .map_err(|error| error.to_string())?;
    if ingested.quota_exceeded {
        return Ok(Accepted::Limited(rate_limited(
            headers,
            ingested.retry_after_secs,
            "organization",
            "quota_exceeded",
        )));
    }
    Ok(Accepted::Stored(ingested.id))
}

async fn read_limited(
    request: Request,
    max: usize,
    headers: &HeaderMap,
) -> Result<Vec<u8>, Response> {
    if let Some(length) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<usize>().ok())
    {
        if length > max {
            return Err(too_large(headers));
        }
    }
    let mut body = request.into_body();
    let mut bytes = Vec::new();
    while let Some(frame) = body.frame().await {
        let frame = match frame {
            Ok(frame) => frame,
            Err(_) => return Err(too_large(headers)),
        };
        let Ok(data) = frame.into_data() else {
            continue;
        };
        if bytes.len().saturating_add(data.len()) > max {
            return Err(too_large(headers));
        }
        bytes.extend_from_slice(&data);
    }
    Ok(bytes)
}

fn too_large(request_headers: &HeaderMap) -> Response {
    ingest_error(
        request_headers,
        StatusCode::PAYLOAD_TOO_LARGE,
        "payload too large".into(),
    )
}

fn rate_limited(request_headers: &HeaderMap, retry_after: u64, scope: &str, reason: &str) -> Response {
    let retry_after = retry_after.max(1);
    let mut response = ingest_error(
        request_headers,
        StatusCode::TOO_MANY_REQUESTS,
        "rate limit exceeded".into(),
    );
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&retry_after.to_string()) {
        headers.insert(header::RETRY_AFTER, value);
    }
    let limit = if reason.is_empty() {
        format!("{retry_after}:error:{scope}")
    } else {
        format!("{retry_after}:error:{scope}:{reason}")
    };
    if let Ok(value) = HeaderValue::from_str(&limit) {
        headers.insert("x-sentry-rate-limits", value);
    }
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("Retry-After, X-Sentry-Rate-Limits, X-Sentry-Error"),
    );
    response
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
