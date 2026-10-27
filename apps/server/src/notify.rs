use std::time::Duration;

use haruvo_domain::{NotificationPayload, sign_body};
use haruvo_platform::{Id, TimestampMillis};
use serde_json::Value;

use crate::repo::{self, Channel};
use crate::state::AppState;

pub async fn fanout(
    state: &AppState,
    payload: &NotificationPayload,
    monitor_id: Option<Id>,
    incident_id: Option<Id>,
) {
    let channels = match repo::channels_for_monitor(&state.db, monitor_id).await {
        Ok(channels) => channels,
        Err(error) => {
            tracing::error!(%error, "loading notification channels");
            return;
        }
    };
    for channel in channels {
        deliver(state, &channel, payload, monitor_id, incident_id).await;
    }
}

pub async fn deliver(
    state: &AppState,
    channel: &Channel,
    payload: &NotificationPayload,
    monitor_id: Option<Id>,
    incident_id: Option<Id>,
) {
    let (status, error) = match send(state, channel, payload).await {
        Ok(SendOutcome::Sent) => ("sent", None),
        Ok(SendOutcome::Skipped(reason)) => ("skipped", Some(reason)),
        Err(reason) => ("failed", Some(reason)),
    };
    if let Err(error) = repo::insert_delivery(
        &state.db,
        Id::new(),
        channel.id,
        monitor_id,
        incident_id,
        &payload.event,
        status,
        error.as_deref(),
        TimestampMillis::now(),
    )
    .await
    {
        tracing::error!(%error, "recording notification delivery");
    }
}

enum SendOutcome {
    Sent,
    Skipped(String),
}

async fn send(
    state: &AppState,
    channel: &Channel,
    payload: &NotificationPayload,
) -> Result<SendOutcome, String> {
    let config: Value = serde_json::from_str(&channel.config_json).unwrap_or(Value::Null);
    let (subject, text) = plain(payload);
    match channel.provider.as_str() {
        "email" => {
            let Some(smtp) = effective_smtp(state).await else {
                return Ok(SendOutcome::Skipped(
                    "SMTP is not configured. Save a mail server in Settings, or set HARUVO_SMTP_HOST and HARUVO_SMTP_FROM.".to_owned(),
                ));
            };
            let to = config
                .get("to")
                .and_then(Value::as_str)
                .ok_or_else(|| "email channel is missing a recipient".to_owned())?;
            send_email(&smtp, to, &subject, &text).await?;
            Ok(SendOutcome::Sent)
        }
        "webhook" => {
            let url = http_url(config.get("url"))?;
            let body = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
            let mut request = state
                .http
                .direct
                .post(url)
                .timeout(Duration::from_secs(10))
                .header("content-type", "application/json")
                .body(body.clone());
            if let Some(secret) = config.get("secret").and_then(Value::as_str) {
                if !secret.is_empty() {
                    let signature = sign_body(secret, &body);
                    let header = format!("sha256={signature}");
                    request = request
                        .header("x-haruvo-signature", header.clone())
                        .header("x-status-signature", header);
                }
            }
            let response = request
                .send()
                .await
                .map_err(|error| trim(&error.to_string()))?;
            if !response.status().is_success() {
                return Err(format!("webhook returned HTTP {}", response.status()));
            }
            Ok(SendOutcome::Sent)
        }
        "slack" => {
            let url = http_url(config.get("url"))?;
            post_json(state, url, serde_json::json!({ "text": text })).await?;
            Ok(SendOutcome::Sent)
        }
        "discord" => {
            let url = http_url(config.get("url"))?;
            let content = text.chars().take(1900).collect::<String>();
            post_json(state, url, serde_json::json!({ "content": content })).await?;
            Ok(SendOutcome::Sent)
        }
        other => Err(format!("unknown provider {other}")),
    }
}

async fn post_json(state: &AppState, url: &str, body: serde_json::Value) -> Result<(), String> {
    let response = state
        .http
        .direct
        .post(url)
        .timeout(Duration::from_secs(10))
        .json(&body)
        .send()
        .await
        .map_err(|error| trim(&error.to_string()))?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("endpoint returned HTTP {}", response.status()))
    }
}

async fn effective_smtp(state: &AppState) -> Option<haruvo_platform::SmtpConfig> {
    match crate::instance_settings::load(&state.db).await {
        Ok(settings) => settings
            .smtp
            .map(haruvo_platform::SmtpConfig::from)
            .or_else(|| state.config.smtp.clone()),
        Err(error) => {
            tracing::error!(%error, "loading instance mail server");
            state.config.smtp.clone()
        }
    }
}

async fn send_email(
    smtp: &haruvo_platform::SmtpConfig,
    to: &str,
    subject: &str,
    text: &str,
) -> Result<(), String> {
    use lettre::message::header::ContentType;
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    let email = Message::builder()
        .from(
            smtp.from
                .parse()
                .map_err(|error: lettre::address::AddressError| error.to_string())?,
        )
        .to(to
            .parse()
            .map_err(|error: lettre::address::AddressError| error.to_string())?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(text.to_owned())
        .map_err(|error| error.to_string())?;
    let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host)
        .map_err(|error| error.to_string())?
        .port(smtp.port);
    if let (Some(username), Some(password)) = (&smtp.username, &smtp.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    builder
        .build()
        .send(email)
        .await
        .map_err(|error| trim(&error.to_string()))?;
    Ok(())
}

fn http_url(value: Option<&Value>) -> Result<&str, String> {
    let url = value.and_then(Value::as_str).unwrap_or("");
    if url.starts_with("https://") || url.starts_with("http://") {
        Ok(url)
    } else {
        Err("notification URL must start with http:// or https://".to_owned())
    }
}

pub fn plain(payload: &NotificationPayload) -> (String, String) {
    let name = payload
        .monitor
        .as_ref()
        .map(|monitor| monitor.name.as_str())
        .unwrap_or("Haruvo");
    let subject = match payload.event.as_str() {
        "monitor.down" => format!("[Haruvo] {name} is down"),
        "monitor.recovered" => format!("[Haruvo] {name} recovered"),
        "incident.updated" => format!("[Haruvo] Incident update: {name}"),
        "notification.test" => format!("[Haruvo] Test notification for {name}"),
        _ => format!("[Haruvo] {name}"),
    };
    let mut lines = vec![subject.clone(), String::new()];
    if let Some(monitor) = &payload.monitor {
        lines.push(format!("Monitor: {} ({})", monitor.name, monitor.url));
        lines.push(format!("State: {}", monitor.state.as_str()));
    }
    if let Some(check) = &payload.check {
        lines.push(format!(
            "Check: {} in {}ms",
            check.status.as_str(),
            check.latency_ms
        ));
        if let Some(error) = &check.error {
            lines.push(format!("Error: {error}"));
        }
    }
    if let Some(incident) = &payload.incident {
        lines.push(format!(
            "Incident: {} ({}, {})",
            incident.title,
            incident.status.as_str(),
            incident.impact.as_str()
        ));
    }
    if let Some(message) = &payload.message {
        lines.push(String::new());
        lines.push(message.clone());
    }
    (subject, lines.join("\n"))
}

fn trim(value: &str) -> String {
    value.chars().take(500).collect()
}
