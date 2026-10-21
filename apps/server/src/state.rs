use haruvo_platform::{
    Config, Database, DatabaseConfig, EnvironmentMode, PasswordError, PasswordService,
};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;

use crate::connectivity::Connectivity;
use crate::migrations;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub config: Arc<Config>,
    pub passwords: PasswordService,
    pub http: HttpClients,
    pub scheduler_id: String,
    pub inflight: Inflight,
    pub connectivity: Connectivity,
    pub insights_secret: String,
    pub collector_limit: Arc<crate::insights::RateLimiter>,
    pub public_limit: Arc<crate::insights::RateLimiter>,
    pub geo: Option<Arc<maxminddb::Reader<Vec<u8>>>>,
}

#[derive(Clone)]
pub struct HttpClients {
    pub follow: reqwest::Client,
    pub direct: reqwest::Client,
}

impl HttpClients {
    pub fn new() -> Result<Self, reqwest::Error> {
        Ok(Self {
            follow: client(reqwest::redirect::Policy::limited(5))?,
            direct: client(reqwest::redirect::Policy::none())?,
        })
    }
}

fn client(policy: reqwest::redirect::Policy) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .redirect(policy)
        .user_agent(concat!("Haruvo/", env!("CARGO_PKG_VERSION")))
        .build()
}

#[derive(Clone, Default)]
pub struct Inflight {
    inner: Arc<Mutex<HashSet<String>>>,
}

pub struct CheckGuard {
    inner: Arc<Mutex<HashSet<String>>>,
    id: String,
}

impl Inflight {
    pub fn try_acquire(&self, id: &str) -> Option<CheckGuard> {
        let mut guard = lock(&self.inner);
        if !guard.insert(id.to_owned()) {
            return None;
        }
        Some(CheckGuard {
            inner: Arc::clone(&self.inner),
            id: id.to_owned(),
        })
    }
}

impl Drop for CheckGuard {
    fn drop(&mut self) {
        lock(&self.inner).remove(&self.id);
    }
}

fn lock(inner: &Mutex<HashSet<String>>) -> MutexGuard<'_, HashSet<String>> {
    inner.lock().unwrap_or_else(|error| error.into_inner())
}

pub async fn boot(config: Config) -> Result<AppState, BootError> {
    let database = Database::connect(&DatabaseConfig {
        path: config.database_path.clone(),
        postgres_url: config.database_url.clone(),
        max_connections: if config.database_url.is_some() { 16 } else { 8 },
    })
    .await?;
    let applied = migrations::runner().run(&database).await?;
    tracing::info!(applied, backend = config.backend_name(), "database ready");
    let passwords = PasswordService::new(config.argon_memory_kib)?;
    let insights = crate::insights::prepare(&database, &config)
        .await
        .map_err(|error| BootError::Config(error.to_string()))?;
    Ok(AppState {
        db: database,
        http: HttpClients::new()?,
        scheduler_id: haruvo_platform::Id::new().to_string(),
        inflight: Inflight::default(),
        connectivity: Connectivity::new(config.connectivity_targets.clone()),
        passwords,
        insights_secret: insights.secret,
        collector_limit: insights.collector_limit,
        public_limit: insights.public_limit,
        geo: insights.geo,
        config: Arc::new(config),
    })
}

pub async fn hash_password(
    passwords: &PasswordService,
    password: String,
) -> Result<String, PasswordError> {
    let passwords = passwords.clone();
    tokio::task::spawn_blocking(move || passwords.hash(&password))
        .await
        .unwrap_or(Err(PasswordError::HashingFailed))
}

pub async fn verify_password(
    passwords: &PasswordService,
    password: String,
    encoded: String,
) -> Result<bool, PasswordError> {
    let passwords = passwords.clone();
    tokio::task::spawn_blocking(move || passwords.verify(&password, &encoded))
        .await
        .unwrap_or(Err(PasswordError::HashingFailed))
}

#[must_use]
pub fn testing_config(path: PathBuf) -> Config {
    Config {
        environment: EnvironmentMode::Development,
        bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port: 0,
        public_origin: "http://127.0.0.1".to_owned(),
        database_path: path,
        database_url: None,
        open_signup: true,
        smtp: None,
        argon_memory_kib: 32,
        connectivity_targets: Vec::new(),
        proxy_hops: 0,
        geoip_path: None,
    }
}

#[derive(Debug, Error)]
pub enum BootError {
    #[error(transparent)]
    Database(#[from] haruvo_platform::DatabaseError),
    #[error(transparent)]
    Migration(#[from] haruvo_platform::MigrationError),
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error("http client: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Config(String),
}
