use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentStatus {
    Investigating,
    Identified,
    Monitoring,
    Resolved,
}

impl IncidentStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Investigating => "investigating",
            Self::Identified => "identified",
            Self::Monitoring => "monitoring",
            Self::Resolved => "resolved",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "investigating" => Self::Investigating,
            "identified" => Self::Identified,
            "monitoring" => Self::Monitoring,
            "resolved" => Self::Resolved,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IncidentImpact {
    None,
    Minor,
    Major,
    Critical,
}

impl IncidentImpact {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Minor => "minor",
            Self::Major => "major",
            Self::Critical => "critical",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "none" => Self::None,
            "minor" => Self::Minor,
            "major" => Self::Major,
            "critical" => Self::Critical,
            _ => return None,
        })
    }
}

/// Resolved incidents keep the original resolution time. Reopening clears it.
#[must_use]
pub fn resolved_at_after_update(
    previous: Option<i64>,
    status: IncidentStatus,
    now: i64,
) -> Option<i64> {
    match status {
        IncidentStatus::Resolved => Some(previous.unwrap_or(now)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_timestamp_sticks_until_reopened() {
        assert_eq!(
            resolved_at_after_update(None, IncidentStatus::Investigating, 10),
            None
        );
        assert_eq!(
            resolved_at_after_update(None, IncidentStatus::Resolved, 10),
            Some(10)
        );
        assert_eq!(
            resolved_at_after_update(Some(10), IncidentStatus::Resolved, 40),
            Some(10)
        );
        assert_eq!(
            resolved_at_after_update(Some(10), IncidentStatus::Monitoring, 40),
            None
        );
    }
}
