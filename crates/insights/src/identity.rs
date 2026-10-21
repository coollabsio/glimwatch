//! Visitor and visit identifiers. A keyed hash of source, address, and user
//! agent rotates monthly. The address itself is never stored.
use sha2::{Digest, Sha256};

/// A visit ends after this much idle time.
pub const VISIT_IDLE: i64 = 30 * 60;

pub fn session_id(secret: &str, source_id: &str, ip: &str, ua: &str, now: i64) -> String {
    let (year, month) = year_month(now);
    hex32(
        Sha256::new()
            .chain_update(secret)
            .chain_update(format!("{year}-{month}"))
            .chain_update(source_id)
            .chain_update(ip)
            .chain_update(ua)
            .finalize(),
    )
}

pub fn visit_id(session_id: &str, started_at: i64) -> String {
    hex32(
        Sha256::new()
            .chain_update(session_id)
            .chain_update(started_at.to_le_bytes())
            .finalize(),
    )
}

fn hex32(digest: impl AsRef<[u8]>) -> String {
    digest.as_ref()[..16]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Civil year and month of a unix timestamp (Howard Hinnant's algorithm).
fn year_month(ts: i64) -> (i64, i64) {
    let z = ts.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month)
}
