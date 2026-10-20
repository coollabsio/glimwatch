use crate::incident::IncidentImpact;
use crate::monitor::MonitorState;

/// The single banner on a public status page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageBanner {
    Operational,
    Degraded,
    PartialOutage,
    MajorOutage,
}

impl PageBanner {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Operational => "operational",
            Self::Degraded => "degraded",
            Self::PartialOutage => "partial_outage",
            Self::MajorOutage => "major_outage",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct OpenIncident {
    pub impact: IncidentImpact,
}

pub fn page_banner(states: &[MonitorState], incidents: &[OpenIncident]) -> PageBanner {
    let mut level = 0_u8;
    let downs = states
        .iter()
        .filter(|state| **state == MonitorState::Down)
        .count();
    if downs > 0 && downs == states.len() {
        level = 3;
    } else if downs > 0 {
        level = 2;
    }
    if states.iter().any(|state| *state == MonitorState::Degraded) {
        level = level.max(1);
    }
    for incident in incidents {
        level = level.max(match incident.impact {
            IncidentImpact::Critical => 3,
            IncidentImpact::Major => 2,
            IncidentImpact::Minor => 1,
            IncidentImpact::None => 0,
        });
    }
    match level {
        3 => PageBanner::MajorOutage,
        2 => PageBanner::PartialOutage,
        1 => PageBanner::Degraded,
        _ => PageBanner::Operational,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_follows_the_worst_signal() {
        assert_eq!(page_banner(&[], &[]), PageBanner::Operational);
        assert_eq!(
            page_banner(&[MonitorState::Up, MonitorState::Down], &[]),
            PageBanner::PartialOutage
        );
        assert_eq!(
            page_banner(&[MonitorState::Down, MonitorState::Down], &[]),
            PageBanner::MajorOutage
        );
        assert_eq!(
            page_banner(
                &[MonitorState::Up],
                &[OpenIncident {
                    impact: IncidentImpact::Critical
                }]
            ),
            PageBanner::MajorOutage
        );
        assert_eq!(
            page_banner(&[MonitorState::Degraded], &[]),
            PageBanner::Degraded
        );
    }
}
