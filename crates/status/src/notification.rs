use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;

use crate::assertion::CheckStatus;
use crate::incident::{IncidentImpact, IncidentStatus};
use crate::monitor::MonitorState;

type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationEvent {
    MonitorDown,
    MonitorRecovered,
    IncidentUpdated,
    Test,
}

impl NotificationEvent {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MonitorDown => "monitor.down",
            Self::MonitorRecovered => "monitor.recovered",
            Self::IncidentUpdated => "incident.updated",
            Self::Test => "notification.test",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NotificationPayload {
    pub event: String,
    pub sent_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitor: Option<PayloadMonitor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<PayloadCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incident: Option<PayloadIncident>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PayloadMonitor {
    pub id: String,
    pub name: String,
    pub url: String,
    pub state: MonitorState,
}

#[derive(Clone, Debug, Serialize)]
pub struct PayloadCheck {
    pub status: CheckStatus,
    pub status_code: Option<i64>,
    pub latency_ms: i64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PayloadIncident {
    pub id: String,
    pub title: String,
    pub status: IncidentStatus,
    pub impact: IncidentImpact,
}

/// Hex-encoded HMAC-SHA256 of the raw body. Webhooks send this as
/// `X-Owlee-Signature: sha256=<hex>`.
#[must_use]
pub fn sign_body(secret: &str, body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key");
    mac.update(body);
    hex_encode(&mac.finalize().into_bytes())
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_changes_with_the_body() {
        let first = sign_body("secret", br#"{"event":"monitor.down"}"#);
        let second = sign_body("secret", br#"{"event":"monitor.recovered"}"#);
        assert_ne!(first, second);
        assert_eq!(first, sign_body("secret", br#"{"event":"monitor.down"}"#));
        assert_eq!(first.len(), 64);
    }
}
