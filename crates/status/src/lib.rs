//! Domain rules for Status.
//!
//! This crate plays the role of Orbit's `orbit-domain`: pure decisions about
//! monitors, assertions, uptime, incidents, and notification payloads. The
//! server crate persists them and performs the network checks.

mod assertion;
mod banner;
mod incident;
mod monitor;
mod notification;
mod slug;
mod uptime;

pub use assertion::{
    Assertion, CheckStatus, Judgement, NumCompare, Probe, Rules, StrCompare, judge,
};
pub use banner::{OpenIncident, PageBanner, page_banner};
pub use incident::{IncidentImpact, IncidentStatus, resolved_at_after_update};
pub use monitor::{
    HeaderPair, JobType, MonitorInput, MonitorState, Transition, apply_outcome, parse_tcp_target,
    validate_monitor,
};
pub use notification::{
    NotificationEvent, NotificationPayload, PayloadCheck, PayloadIncident, PayloadMonitor,
    sign_body,
};
pub use slug::{SlugError, validate_slug};
pub use uptime::{DailyCount, DayBucket, DayStatus, WindowStats, summarize};
