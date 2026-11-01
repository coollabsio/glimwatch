//! In-memory token bucket for a single Owlee process.
//!
//! Public ingest is limited per project or site key and per client address.
//! A second node does not share these buckets.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

const MAX_KEYS: usize = 10_000;

#[derive(Debug, Clone, Copy)]
pub struct RateDecision {
    pub allowed: bool,
    pub retry_after_secs: u64,
}

pub struct TokenBucket {
    per_sec: f64,
    burst: f64,
    buckets: Mutex<HashMap<String, (Instant, f64)>>,
}

impl TokenBucket {
    pub fn per_minute(per_minute: u32, burst: u32) -> Self {
        let per_minute = per_minute.max(1);
        let burst = burst.max(1);
        Self {
            per_sec: f64::from(per_minute) / 60.0,
            burst: f64::from(burst),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    pub fn check(&self, key: &str) -> RateDecision {
        let now = Instant::now();
        let mut buckets = self
            .buckets
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if buckets.len() >= MAX_KEYS && !buckets.contains_key(key) {
            buckets.retain(|_, (updated, tokens)| {
                now.duration_since(*updated).as_secs() < 600 || *tokens >= 1.0
            });
            if buckets.len() >= MAX_KEYS {
                buckets.clear();
            }
        }
        let entry = buckets
            .entry(key.to_owned())
            .or_insert((now, self.burst));
        let elapsed = now.duration_since(entry.0).as_secs_f64();
        entry.0 = now;
        entry.1 = (entry.1 + elapsed * self.per_sec).min(self.burst);
        if entry.1 >= 1.0 {
            entry.1 -= 1.0;
            RateDecision {
                allowed: true,
                retry_after_secs: 0,
            }
        } else {
            let need = 1.0 - entry.1;
            let secs = (need / self.per_sec).ceil().max(1.0) as u64;
            RateDecision {
                allowed: false,
                retry_after_secs: secs,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_then_retry_after() {
        let bucket = TokenBucket::per_minute(60, 2);
        assert!(bucket.check("project").allowed);
        assert!(bucket.check("project").allowed);
        let denied = bucket.check("project");
        assert!(!denied.allowed);
        assert!(denied.retry_after_secs >= 1);
    }
}
