//! DNS TXT ownership for analytics domains.
//!
//! Record: `_owlee-verify.example.com` TXT `owlee-verify=<token>`.
//!
//! Unverified domains may collect only while `OWLEE_ENV` is development (or
//! unset, which is development). Production refuses the event until Verify
//! succeeds. A later re-check clears `verified` when DNS answers and the
//! token is gone. A lookup error leaves the flag alone so a resolver blip
//! does not drop traffic. Status pages have no custom hostname.

use owlee_insights::now_secs;
use sqlx::Row;

use crate::error::AppError;
use crate::state::AppState;

pub fn record_name(domain: &str) -> String {
    format!("_owlee-verify.{domain}")
}

pub fn record_value(token: &str) -> String {
    format!("owlee-verify={token}")
}

pub fn txt_matches(records: &[String], token: &str) -> bool {
    let expected = record_value(token);
    records.iter().any(|record| {
        let record = record.trim().trim_matches('"');
        record == expected || record.split_whitespace().any(|part| part == expected)
    })
}

pub async fn verify_site(state: &AppState, id: &str) -> Result<bool, AppError> {
    let row = sqlx::query("SELECT domain, verify_token FROM insight_sites WHERE id = $1")
        .bind(id)
        .fetch_optional(state.db.pool())
        .await?
        .ok_or(AppError::NotFound)?;
    let domain: String = row.try_get("domain")?;
    let token: String = row.try_get("verify_token")?;
    if token.is_empty() {
        return Err(AppError::BadRequest(
            "This site has no verification token yet.".into(),
        ));
    }
    let name = record_name(&domain);
    let records = state.dns.lookup_txt(&name).await.map_err(|error| {
        tracing::warn!(%error, %name, "domain verification lookup failed");
        AppError::BadRequest(format!("DNS lookup for {name} failed."))
    })?;
    let ok = txt_matches(&records, &token);
    let now = now_secs();
    if ok {
        sqlx::query(
            "UPDATE insight_sites SET verified = 1, verified_at = $1 WHERE id = $2",
        )
        .bind(now)
        .bind(id)
        .execute(state.db.pool())
        .await?;
    } else {
        sqlx::query("UPDATE insight_sites SET verified = 0, verified_at = NULL WHERE id = $1")
            .bind(id)
            .execute(state.db.pool())
            .await?;
    }
    Ok(ok)
}

/// Hourly pass. Lookup failures are logged and do not clear a verified domain.
pub async fn recheck_verified(state: &AppState) -> Result<usize, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, domain, verify_token FROM insight_sites WHERE verified = 1 AND verify_token != ''",
    )
    .fetch_all(state.db.pool())
    .await?;
    let mut cleared = 0;
    for row in &rows {
        let id: String = row.try_get("id")?;
        let domain: String = row.try_get("domain")?;
        let token: String = row.try_get("verify_token")?;
        let name = record_name(&domain);
        match state.dns.lookup_txt(&name).await {
            Ok(records) if txt_matches(&records, &token) => {}
            Ok(_) => {
                sqlx::query(
                    "UPDATE insight_sites SET verified = 0, verified_at = NULL WHERE id = $1",
                )
                .bind(&id)
                .execute(state.db.pool())
                .await?;
                cleared += 1;
                tracing::info!(%domain, "domain verification lapsed");
            }
            Err(error) => {
                tracing::warn!(%error, %domain, "skipped domain re-check");
            }
        }
    }
    Ok(cleared)
}
