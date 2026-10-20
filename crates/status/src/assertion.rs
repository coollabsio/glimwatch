use serde::{Deserialize, Serialize};

use crate::monitor::JobType;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumCompare {
    Eq,
    NotEq,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrCompare {
    Contains,
    NotContains,
    Eq,
    NotEq,
    Empty,
    NotEmpty,
}

/// A response assertion, a subset of the checks OpenStatus evaluates.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Assertion {
    Status {
        compare: NumCompare,
        target: i64,
    },
    Body {
        compare: StrCompare,
        target: String,
    },
    Header {
        key: String,
        compare: StrCompare,
        target: String,
    },
    Latency {
        compare: NumCompare,
        target: i64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Success,
    Degraded,
    Failure,
    Maintenance,
}

impl CheckStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Degraded => "degraded",
            Self::Failure => "failure",
            Self::Maintenance => "maintenance",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "success" => Self::Success,
            "degraded" => Self::Degraded,
            "failure" => Self::Failure,
            "maintenance" => Self::Maintenance,
            _ => return None,
        })
    }
}

/// What the checker observed. Transport failures set `transport_error` and
/// leave the status code empty.
#[derive(Clone, Debug, Default)]
pub struct Probe {
    pub status_code: Option<i64>,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub latency_ms: i64,
    pub transport_error: Option<String>,
}

/// The rules a monitor contributes to a judgement.
#[derive(Clone, Debug)]
pub struct Rules {
    pub job_type: JobType,
    pub expected_status: Option<i64>,
    pub degraded_after_ms: Option<i64>,
    pub assertions: Vec<Assertion>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Judgement {
    pub status: CheckStatus,
    pub error: Option<String>,
}

pub fn judge(rules: &Rules, probe: &Probe) -> Judgement {
    if let Some(error) = &probe.transport_error {
        return Judgement {
            status: CheckStatus::Failure,
            error: Some(error.clone()),
        };
    }

    let mut failures = Vec::new();
    if rules.job_type == JobType::Http {
        match (rules.expected_status, probe.status_code) {
            (Some(expected), Some(actual)) if actual != expected => {
                failures.push(format!("expected HTTP {expected}, got {actual}"));
            }
            (Some(expected), None) => {
                failures.push(format!("expected HTTP {expected}, got no status"));
            }
            (None, Some(actual)) if !(200..300).contains(&actual) => {
                failures.push(format!("HTTP {actual} is outside the 2xx range"));
            }
            (None, None) => failures.push("HTTP check returned no status".to_owned()),
            _ => {}
        }
    }

    for assertion in &rules.assertions {
        if let Some(message) = evaluate(assertion, probe) {
            failures.push(message);
        }
    }

    if !failures.is_empty() {
        return Judgement {
            status: CheckStatus::Failure,
            error: Some(failures.join("; ")),
        };
    }

    if let Some(limit) = rules.degraded_after_ms
        && probe.latency_ms > limit
    {
        return Judgement {
            status: CheckStatus::Degraded,
            error: Some(format!(
                "latency {}ms is above the degraded threshold of {limit}ms",
                probe.latency_ms
            )),
        };
    }

    Judgement {
        status: CheckStatus::Success,
        error: None,
    }
}

fn evaluate(assertion: &Assertion, probe: &Probe) -> Option<String> {
    match assertion {
        Assertion::Status { compare, target } => {
            let actual = probe.status_code.unwrap_or(0);
            compare_number(actual, *compare, *target).map(|reason| format!("status {reason}"))
        }
        Assertion::Body { compare, target } => {
            compare_string(&probe.body, *compare, target).map(|reason| format!("body {reason}"))
        }
        Assertion::Header {
            key,
            compare,
            target,
        } => {
            let value = header_value(&probe.headers, key);
            compare_string(&value, *compare, target).map(|reason| format!("header {key} {reason}"))
        }
        Assertion::Latency { compare, target } => {
            compare_number(probe.latency_ms, *compare, *target)
                .map(|reason| format!("latency {reason}"))
        }
    }
}

fn header_value(headers: &[(String, String)], key: &str) -> String {
    headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

fn compare_number(actual: i64, compare: NumCompare, target: i64) -> Option<String> {
    let ok = match compare {
        NumCompare::Eq => actual == target,
        NumCompare::NotEq => actual != target,
        NumCompare::Gt => actual > target,
        NumCompare::Gte => actual >= target,
        NumCompare::Lt => actual < target,
        NumCompare::Lte => actual <= target,
    };
    if ok {
        None
    } else {
        Some(format!("expected {actual} {compare:?} {target}"))
    }
}

fn compare_string(actual: &str, compare: StrCompare, target: &str) -> Option<String> {
    let ok = match compare {
        StrCompare::Contains => actual.contains(target),
        StrCompare::NotContains => !actual.contains(target),
        StrCompare::Eq => actual == target,
        StrCompare::NotEq => actual != target,
        StrCompare::Empty => actual.is_empty(),
        StrCompare::NotEmpty => !actual.is_empty(),
    };
    if ok {
        None
    } else {
        Some(format!("failed {compare:?} comparison"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn http_probe(status: i64, body: &str, latency: i64) -> Probe {
        Probe {
            status_code: Some(status),
            headers: vec![("content-type".to_owned(), "application/json".to_owned())],
            body: body.to_owned(),
            latency_ms: latency,
            transport_error: None,
        }
    }

    #[test]
    fn two_xx_is_success_and_slow_is_degraded() {
        let rules = Rules {
            job_type: JobType::Http,
            expected_status: None,
            degraded_after_ms: Some(500),
            assertions: Vec::new(),
        };
        assert_eq!(
            judge(&rules, &http_probe(204, "", 20)).status,
            CheckStatus::Success
        );
        assert_eq!(
            judge(&rules, &http_probe(200, "", 800)).status,
            CheckStatus::Degraded
        );
        assert_eq!(
            judge(&rules, &http_probe(503, "", 10)).status,
            CheckStatus::Failure
        );
    }

    #[test]
    fn assertions_and_expected_status() {
        let rules = Rules {
            job_type: JobType::Http,
            expected_status: Some(201),
            degraded_after_ms: None,
            assertions: vec![
                Assertion::Body {
                    compare: StrCompare::Contains,
                    target: "ok".to_owned(),
                },
                Assertion::Header {
                    key: "Content-Type".to_owned(),
                    compare: StrCompare::Contains,
                    target: "json".to_owned(),
                },
            ],
        };
        let mut probe = http_probe(201, "ok", 10);
        assert_eq!(judge(&rules, &probe).status, CheckStatus::Success);
        probe.body = "nope".to_owned();
        let judgement = judge(&rules, &probe);
        assert_eq!(judgement.status, CheckStatus::Failure);
        assert!(judgement.error.unwrap().contains("body"));
    }

    #[test]
    fn transport_errors_fail_closed() {
        let rules = Rules {
            job_type: JobType::Tcp,
            expected_status: None,
            degraded_after_ms: None,
            assertions: Vec::new(),
        };
        let probe = Probe {
            transport_error: Some("connection refused".to_owned()),
            ..Probe::default()
        };
        assert_eq!(judge(&rules, &probe).status, CheckStatus::Failure);
    }
}
