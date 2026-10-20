use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::assertion::{Assertion, CheckStatus};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobType {
    Http,
    Tcp,
}

impl JobType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::Tcp => "tcp",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "http" => Self::Http,
            "tcp" => Self::Tcp,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MonitorState {
    Pending,
    Up,
    Degraded,
    Down,
}

impl MonitorState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Up => "up",
            Self::Degraded => "degraded",
            Self::Down => "down",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "pending" => Self::Pending,
            "up" => Self::Up,
            "degraded" => Self::Degraded,
            "down" => Self::Down,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HeaderPair {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct MonitorInput {
    pub name: String,
    pub description: String,
    pub job_type: JobType,
    pub url: String,
    pub method: String,
    pub headers: Vec<HeaderPair>,
    pub body: String,
    pub timeout_ms: i64,
    pub interval_seconds: i64,
    pub degraded_after_ms: Option<i64>,
    pub expected_status: Option<i64>,
    pub assertions: Vec<Assertion>,
    pub follow_redirects: bool,
    pub failure_threshold: i32,
    pub active: bool,
}

pub fn validate_monitor(input: &MonitorInput) -> Result<(), BTreeMap<String, String>> {
    let mut errors = BTreeMap::new();
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        errors.insert("name".to_owned(), "Use 1 to 120 characters.".to_owned());
    }
    if input.description.chars().count() > 2_000 {
        errors.insert(
            "description".to_owned(),
            "Keep the description under 2000 characters.".to_owned(),
        );
    }
    if !(1_000..=120_000).contains(&input.timeout_ms) {
        errors.insert(
            "timeout_ms".to_owned(),
            "Timeout must be between 1 and 120 seconds.".to_owned(),
        );
    }
    if !(15..=86_400).contains(&input.interval_seconds) {
        errors.insert(
            "interval_seconds".to_owned(),
            "Interval must be between 15 seconds and 24 hours.".to_owned(),
        );
    }
    if !(1..=20).contains(&input.failure_threshold) {
        errors.insert(
            "failure_threshold".to_owned(),
            "Failure threshold must be between 1 and 20.".to_owned(),
        );
    }
    match input.job_type {
        JobType::Http => {
            if !(input.url.starts_with("http://") || input.url.starts_with("https://"))
                || input.url.chars().count() > 2_048
            {
                errors.insert(
                    "url".to_owned(),
                    "Enter an http or https URL, up to 2048 characters.".to_owned(),
                );
            }
            if !matches!(
                input.method.as_str(),
                "GET" | "POST" | "HEAD" | "PUT" | "PATCH" | "DELETE" | "OPTIONS"
            ) {
                errors.insert("method".to_owned(), "Unsupported HTTP method.".to_owned());
            }
        }
        JobType::Tcp => {
            if parse_tcp_target(&input.url).is_none() {
                errors.insert(
                    "url".to_owned(),
                    "Enter a TCP target as host:port, for example db.internal:5432.".to_owned(),
                );
            }
        }
    }
    if let Some(status) = input.expected_status
        && !(100..=599).contains(&status)
    {
        errors.insert(
            "expected_status".to_owned(),
            "Expected status must be an HTTP status code.".to_owned(),
        );
    }
    if let Some(limit) = input.degraded_after_ms
        && !(1..=120_000).contains(&limit)
    {
        errors.insert(
            "degraded_after_ms".to_owned(),
            "Degraded latency must be between 1ms and 120 seconds.".to_owned(),
        );
    }
    if input.assertions.len() > 20 {
        errors.insert(
            "assertions".to_owned(),
            "A monitor can have at most 20 assertions.".to_owned(),
        );
    }
    if input.headers.len() > 40 {
        errors.insert(
            "headers".to_owned(),
            "A monitor can send at most 40 headers.".to_owned(),
        );
    }
    for header in &input.headers {
        if header.name.trim().is_empty()
            || header.name.contains(['\r', '\n', ':'])
            || header.value.contains(['\r', '\n'])
        {
            errors.insert(
                "headers".to_owned(),
                "Header names and values cannot contain new lines.".to_owned(),
            );
            break;
        }
    }
    if input.body.len() > 65_536 {
        errors.insert(
            "body".to_owned(),
            "Request body must be at most 64 KiB.".to_owned(),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// `host:port` with a numeric port. IPv6 targets use `[addr]:port`.
#[must_use]
pub fn parse_tcp_target(value: &str) -> Option<(String, u16)> {
    let value = value.trim().trim_start_matches("tcp://");
    if let Some(rest) = value.strip_prefix('[') {
        let (host, port) = rest.split_once("]:")?;
        if host.is_empty() {
            return None;
        }
        let port = port.parse::<u16>().ok()?;
        if port == 0 {
            return None;
        }
        return Some((host.to_owned(), port));
    }
    let (host, port) = value.rsplit_once(':')?;
    if host.is_empty() || host.contains(':') {
        return None;
    }
    let port = port.parse::<u16>().ok()?;
    if port == 0 {
        return None;
    }
    Some((host.to_owned(), port))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Transition {
    pub state: MonitorState,
    pub consecutive_failures: i32,
    pub open_incident: bool,
    pub resolve_incident: bool,
}

/// Advances a monitor after one check.
///
/// Failures do not flap the monitor to down until `threshold` consecutive
/// failures. A success or a degraded (slow but reachable) result clears the
/// counter. Only an automatic incident opened for a down monitor is a
/// candidate to resolve, and only the server decides whether one exists.
pub fn apply_outcome(
    previous: MonitorState,
    consecutive_failures: i32,
    threshold: i32,
    outcome: CheckStatus,
) -> Transition {
    let threshold = threshold.max(1);
    match outcome {
        CheckStatus::Maintenance => Transition {
            state: previous,
            consecutive_failures,
            open_incident: false,
            resolve_incident: false,
        },
        CheckStatus::Success => Transition {
            state: MonitorState::Up,
            consecutive_failures: 0,
            open_incident: false,
            resolve_incident: previous == MonitorState::Down,
        },
        CheckStatus::Degraded => Transition {
            state: MonitorState::Degraded,
            consecutive_failures: 0,
            open_incident: false,
            resolve_incident: previous == MonitorState::Down,
        },
        CheckStatus::Failure => {
            let failures = consecutive_failures.saturating_add(1);
            if failures >= threshold {
                Transition {
                    state: MonitorState::Down,
                    consecutive_failures: failures,
                    open_incident: previous != MonitorState::Down,
                    resolve_incident: false,
                }
            } else {
                Transition {
                    state: previous,
                    consecutive_failures: failures,
                    open_incident: false,
                    resolve_incident: false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_prevents_flapping_and_recovery_resolves() {
        let once = apply_outcome(MonitorState::Up, 0, 3, CheckStatus::Failure);
        assert_eq!(once.state, MonitorState::Up);
        assert_eq!(once.consecutive_failures, 1);
        assert!(!once.open_incident);

        let second = apply_outcome(MonitorState::Up, 2, 3, CheckStatus::Failure);
        assert_eq!(second.state, MonitorState::Down);
        assert!(second.open_incident);

        let again = apply_outcome(MonitorState::Down, 3, 3, CheckStatus::Failure);
        assert!(!again.open_incident);

        let back = apply_outcome(MonitorState::Down, 4, 3, CheckStatus::Success);
        assert_eq!(back.state, MonitorState::Up);
        assert!(back.resolve_incident);
        assert_eq!(back.consecutive_failures, 0);
    }

    #[test]
    fn maintenance_does_not_move_state() {
        let transition = apply_outcome(MonitorState::Up, 2, 3, CheckStatus::Maintenance);
        assert_eq!(transition.state, MonitorState::Up);
        assert_eq!(transition.consecutive_failures, 2);
    }

    #[test]
    fn validates_http_and_tcp_targets() {
        let mut input = sample();
        assert!(validate_monitor(&input).is_ok());
        input.url = "ftp://example.com".to_owned();
        assert!(validate_monitor(&input).is_err());
        input.job_type = JobType::Tcp;
        input.url = "db.internal:5432".to_owned();
        assert!(validate_monitor(&input).is_ok());
        assert_eq!(parse_tcp_target("[::1]:443"), Some(("::1".to_owned(), 443)));
    }

    fn sample() -> MonitorInput {
        MonitorInput {
            name: "API".to_owned(),
            description: String::new(),
            job_type: JobType::Http,
            url: "https://example.com/health".to_owned(),
            method: "GET".to_owned(),
            headers: Vec::new(),
            body: String::new(),
            timeout_ms: 10_000,
            interval_seconds: 30,
            degraded_after_ms: None,
            expected_status: None,
            assertions: Vec::new(),
            follow_redirects: true,
            failure_threshold: 3,
            active: true,
        }
    }
}
