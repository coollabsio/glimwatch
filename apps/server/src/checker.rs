use std::time::{Duration, Instant};

use status_domain::{Probe, parse_tcp_target};
use tokio::net::TcpStream;

use crate::repo::Monitor;
use crate::state::HttpClients;

const BODY_LIMIT: usize = 65_536;

pub async fn probe(http: &HttpClients, monitor: &Monitor) -> Probe {
    let started = Instant::now();
    let timeout = Duration::from_millis(monitor.timeout_ms.max(1) as u64);
    match monitor.job_type {
        status_domain::JobType::Http => http_probe(http, monitor, timeout, started).await,
        status_domain::JobType::Tcp => tcp_probe(monitor, timeout, started).await,
    }
}

async fn http_probe(
    http: &HttpClients,
    monitor: &Monitor,
    timeout: Duration,
    started: Instant,
) -> Probe {
    let client = if monitor.follow_redirects {
        &http.follow
    } else {
        &http.direct
    };
    let method =
        reqwest::Method::from_bytes(monitor.method.as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut request = client
        .request(method.clone(), &monitor.url)
        .timeout(timeout);
    let mut headers = reqwest::header::HeaderMap::new();
    for header in &monitor.headers {
        let Ok(name) = reqwest::header::HeaderName::from_bytes(header.name.as_bytes()) else {
            continue;
        };
        let Ok(value) = reqwest::header::HeaderValue::from_str(&header.value) else {
            continue;
        };
        headers.insert(name, value);
    }
    request = request.headers(headers);
    if matches!(
        method,
        reqwest::Method::POST | reqwest::Method::PUT | reqwest::Method::PATCH
    ) && !monitor.body.is_empty()
    {
        request = request.body(monitor.body.clone());
    }
    let response = match request.send().await {
        Ok(response) => response,
        Err(error) => {
            return Probe {
                latency_ms: elapsed_ms(started),
                transport_error: Some(trim_error(&error.to_string())),
                ..Probe::default()
            };
        }
    };
    let status_code = i64::from(response.status().as_u16());
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_owned(),
                String::from_utf8_lossy(value.as_bytes()).into_owned(),
            )
        })
        .collect();
    let mut body = Vec::new();
    let mut response = response;
    loop {
        if body.len() >= BODY_LIMIT {
            break;
        }
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let room = BODY_LIMIT - body.len();
                let take = chunk.len().min(room);
                body.extend_from_slice(&chunk[..take]);
            }
            Ok(None) => break,
            Err(error) => {
                return Probe {
                    status_code: Some(status_code),
                    headers,
                    latency_ms: elapsed_ms(started),
                    transport_error: Some(trim_error(&error.to_string())),
                    ..Probe::default()
                };
            }
        }
    }
    Probe {
        status_code: Some(status_code),
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
        latency_ms: elapsed_ms(started),
        transport_error: None,
    }
}

async fn tcp_probe(monitor: &Monitor, timeout: Duration, started: Instant) -> Probe {
    let Some((host, port)) = parse_tcp_target(&monitor.url) else {
        return Probe {
            latency_ms: 0,
            transport_error: Some("TCP target must look like host:port.".to_owned()),
            ..Probe::default()
        };
    };
    match tokio::time::timeout(timeout, TcpStream::connect((host.as_str(), port))).await {
        Ok(Ok(stream)) => {
            drop(stream);
            Probe {
                latency_ms: elapsed_ms(started),
                ..Probe::default()
            }
        }
        Ok(Err(error)) => Probe {
            latency_ms: elapsed_ms(started),
            transport_error: Some(trim_error(&error.to_string())),
            ..Probe::default()
        },
        Err(_) => Probe {
            latency_ms: elapsed_ms(started),
            transport_error: Some(format!("timed out after {}ms", timeout.as_millis())),
            ..Probe::default()
        },
    }
}

fn elapsed_ms(started: Instant) -> i64 {
    i64::try_from(started.elapsed().as_millis()).unwrap_or(i64::MAX)
}

fn trim_error(error: &str) -> String {
    let mut chars = error.chars();
    let trimmed = chars.by_ref().take(500).collect::<String>();
    if chars.next().is_some() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
