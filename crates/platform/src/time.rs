use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A UTC timestamp stored as Unix milliseconds.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TimestampMillis(i64);

impl TimestampMillis {
    #[must_use]
    pub const fn from_millis(milliseconds: i64) -> Self {
        Self(milliseconds)
    }

    #[must_use]
    pub const fn as_millis(self) -> i64 {
        self.0
    }

    #[must_use]
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or(0);
        Self(millis)
    }

    /// Unix day number (`millis / 86_400_000`), used for uptime buckets.
    #[must_use]
    pub const fn unix_day(self) -> i64 {
        self.0.div_euclid(86_400_000)
    }

    fn as_datetime(self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp_millis(self.0)
    }
}

impl fmt::Display for TimestampMillis {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_datetime() {
            Some(value) => formatter.write_str(&value.to_rfc3339_opts(SecondsFormat::Millis, true)),
            None => write!(formatter, "{}ms since Unix epoch", self.0),
        }
    }
}

impl Serialize for TimestampMillis {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let value = self
            .as_datetime()
            .ok_or_else(|| serde::ser::Error::custom("timestamp is outside the RFC 3339 range"))?;
        serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Millis, true))
    }
}

impl<'de> Deserialize<'de> for TimestampMillis {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let parsed = DateTime::parse_from_rfc3339(&value).map_err(serde::de::Error::custom)?;
        if parsed.timestamp_subsec_nanos() % 1_000_000 != 0 {
            return Err(serde::de::Error::custom(
                "timestamp has precision finer than milliseconds",
            ));
        }
        Ok(Self(parsed.timestamp_millis()))
    }
}
