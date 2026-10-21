//! Cookieless analytics and short-link rules.
//!
//! The behaviour follows [Beacon](https://github.com/ShadowArcanist/beacons) by
//! ShadowArcanist (MIT). Visitor ids are a monthly keyed hash of the client
//! address and user agent, the same idea Umami uses. The address is never stored.

mod event;
mod identity;
mod text;
mod ua;
mod url;

pub use event::{
    DataValue, EventPayload, Incoming, TYPE_ARRAY, TYPE_BOOLEAN, TYPE_NUMBER, TYPE_STRING, Vitals,
    clean_vitals, flatten,
};
pub use identity::{VISIT_IDLE, session_id, visit_id};
pub use text::{
    clean_color, clean_destination, clean_host, clean_name, clean_public_slug, clean_slug, slugify,
};
pub use ua::parse_user_agent;
pub use url::{PageUrl, clip, parse as parse_page, referrer_domain, same_site};

pub fn day(timestamp: i64) -> i64 {
    timestamp - timestamp.rem_euclid(86_400)
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub from: i64,
    pub to: i64,
    pub first_day: i64,
    pub last_day: i64,
}

impl Bounds {
    /// `from` and `to` are unix seconds. The end is exclusive. Missing values
    /// mean the last 30 days, and the window is capped at 366 days.
    pub fn resolve(from: Option<i64>, to: Option<i64>, now: i64) -> Self {
        let now = now.max(1);
        let to = to.unwrap_or(now).clamp(1, now);
        let from = from
            .unwrap_or(to - 30 * 86_400)
            .clamp(to.saturating_sub(366 * 86_400), to - 1);
        let to = to.max(from + 1);
        Self {
            from,
            to,
            first_day: day(from),
            last_day: day(to - 1),
        }
    }
}

/// Nearest-rank percentile. `sorted` must already be in ascending order.
pub fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let index = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(index.min(sorted.len() - 1)).copied()
}

pub const SITE_SECTIONS: &[&str] = &[
    "pageviews",
    "visitors",
    "path",
    "referrer",
    "country",
    "browser",
    "os",
    "device",
];

pub const LINK_SECTIONS: &[&str] = &[
    "clicks", "visitors", "referrer", "country", "browser", "os", "device",
];

pub fn sections_json(keys: &[&str]) -> String {
    serde_json::to_string(keys).unwrap_or_else(|_| "[]".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_is_utc_midnight() {
        assert_eq!(day(86_400 + 10), 86_400);
        assert_eq!(day(-10), -86_400);
    }

    #[test]
    fn session_id_rotates_by_month_and_hides_the_address() {
        let left = session_id("secret", "site", "203.0.113.4", "Mozilla", 1_700_000_000);
        let right = session_id("secret", "site", "203.0.113.4", "Mozilla", 1_700_000_000);
        assert_eq!(left, right);
        assert!(!left.contains("203.0.113.4"));
        assert_ne!(
            left,
            session_id("secret", "site", "203.0.113.5", "Mozilla", 1_700_000_000)
        );
    }

    #[test]
    fn parses_a_page_and_drops_the_sites_own_referrer() {
        let page = url::parse("https://coolify.io/docs?utm_source=github#install").unwrap();
        assert_eq!(page.path, "/docs#install");
        assert_eq!(page.utm[0].as_deref(), Some("github"));
        assert_eq!(
            referrer_domain("https://www.Google.com/search"),
            Some("google.com".into())
        );
        assert!(same_site("docs.coolify.io", "coolify.io"));
        assert!(!same_site("coolify.io.evil.test", "coolify.io"));
    }
}
