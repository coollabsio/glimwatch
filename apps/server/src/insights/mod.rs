//! Cookieless page analytics and the short-link redirector.
//!
//! The tracker wire format (`/data.js`, `POST /api/receive`) matches
//! ShadowArcanist's Beacon so an existing snippet keeps working. Dashboard
//! routes use Haruvo's session cookie.

mod geo;
mod limit;
mod record;
mod routes;
mod seed;

use std::sync::Arc;
use std::time::Duration;

use haruvo_platform::{Config, Database, generate_token};
use maxminddb::Reader;
use sqlx::Row;

pub use limit::RateLimiter;
pub use routes::{api_routes, fallback, public_routes};
pub use seed::seed_demo;

pub struct Runtime {
    pub secret: String,
    pub collector_limit: Arc<RateLimiter>,
    pub public_limit: Arc<RateLimiter>,
    pub geo: Option<Arc<Reader<Vec<u8>>>>,
}

pub async fn prepare(db: &Database, config: &Config) -> Result<Runtime, sqlx::Error> {
    Ok(Runtime {
        secret: ensure_secret(db).await?,
        collector_limit: Arc::new(RateLimiter::new(40, Duration::from_secs(60))),
        public_limit: Arc::new(RateLimiter::new(60, Duration::from_secs(60))),
        geo: config
            .geoip_path
            .as_ref()
            .and_then(|path| geo::open_reader(path).map(Arc::new)),
    })
}

async fn ensure_secret(db: &Database) -> Result<String, sqlx::Error> {
    let fresh = generate_token();
    sqlx::query(
        "INSERT INTO insight_settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO NOTHING",
    )
    .bind("secret")
    .bind(&fresh)
    .execute(db.pool())
    .await?;
    let row = sqlx::query("SELECT value FROM insight_settings WHERE key = $1")
        .bind("secret")
        .fetch_one(db.pool())
        .await?;
    row.try_get("value")
}
