use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use thiserror::Error;

const ENV: &str = "GLIMWATCH_ENV";
const BIND: &str = "GLIMWATCH_BIND";
const PORT: &str = "GLIMWATCH_PORT";
const ORIGIN: &str = "GLIMWATCH_PUBLIC_ORIGIN";
const DATABASE_PATH: &str = "GLIMWATCH_DATABASE_PATH";
const DATABASE_URL: &str = "GLIMWATCH_DATABASE_URL";
const DATABASE_URL_ALIAS: &str = "DATABASE_URL";
const OPEN_SIGNUP: &str = "GLIMWATCH_OPEN_SIGNUP";
const SMTP_HOST: &str = "GLIMWATCH_SMTP_HOST";
const SMTP_PORT: &str = "GLIMWATCH_SMTP_PORT";
const SMTP_USERNAME: &str = "GLIMWATCH_SMTP_USERNAME";
const SMTP_PASSWORD: &str = "GLIMWATCH_SMTP_PASSWORD";
const SMTP_FROM: &str = "GLIMWATCH_SMTP_FROM";
const ARGON_MEMORY: &str = "GLIMWATCH_ARGON_MEMORY_KIB";
const CONNECTIVITY_TARGETS: &str = "GLIMWATCH_CONNECTIVITY_TARGETS";
const PROXY_HOPS: &str = "GLIMWATCH_PROXY_HOPS";
const GEOIP_PATH: &str = "GLIMWATCH_GEOIP_PATH";

#[derive(Clone, Debug)]
pub struct Config {
    pub environment: EnvironmentMode,
    pub bind: IpAddr,
    pub port: u16,
    pub public_origin: String,
    pub database_path: PathBuf,
    pub database_url: Option<String>,
    pub open_signup: bool,
    pub smtp: Option<SmtpConfig>,
    pub argon_memory_kib: u32,
    /// `host:port` targets that prove the Glimwatch host itself has network.
    /// Empty disables the check.
    pub connectivity_targets: Vec<String>,
    /// Trusted reverse proxies in front of Glimwatch. `0` means the socket address is the client.
    pub proxy_hops: usize,
    /// Optional MaxMind or DB-IP city database (`.mmdb`) for analytics geolocation.
    pub geoip_path: Option<PathBuf>,
}

impl Config {
    #[must_use]
    pub fn development() -> Self {
        Self {
            environment: EnvironmentMode::Development,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 43123,
            public_origin: "http://127.0.0.1:43123".to_owned(),
            database_path: PathBuf::from("data/glimwatch.sqlite"),
            database_url: None,
            open_signup: false,
            smtp: None,
            argon_memory_kib: 19456,
            connectivity_targets: ["1.1.1.1:443", "8.8.8.8:443", "9.9.9.9:443"]
                .map(str::to_owned)
                .to_vec(),
            proxy_hops: 0,
            geoip_path: None,
        }
    }

    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = match optional(ENV).as_deref() {
            None => EnvironmentMode::Development,
            Some("development") => EnvironmentMode::Development,
            Some("production") => EnvironmentMode::Production,
            Some(other) => {
                return Err(ConfigError::Invalid {
                    name: ENV,
                    value: other.to_owned(),
                });
            }
        };
        let mut config = Self::development();
        config.environment = environment;
        if environment == EnvironmentMode::Production {
            config.open_signup = false;
            config.bind = IpAddr::V4(Ipv4Addr::UNSPECIFIED);
        }
        if let Some(bind) = optional(BIND) {
            config.bind = bind.parse().map_err(|_| ConfigError::Invalid {
                name: BIND,
                value: bind,
            })?;
        }
        if let Some(port) = optional(PORT).or_else(|| optional("PORT")) {
            config.port = port.parse().map_err(|_| ConfigError::Invalid {
                name: PORT,
                value: port,
            })?;
        }
        config.public_origin = optional(ORIGIN).unwrap_or_else(|| {
            let host = match config.bind {
                IpAddr::V4(ip) if ip.is_unspecified() => "127.0.0.1".to_owned(),
                other => other.to_string(),
            };
            format!("http://{host}:{}", config.port)
        });
        config.database_path = resolve_sqlite_path(optional(DATABASE_PATH));
        if let Some(url) = optional(DATABASE_URL).or_else(|| optional(DATABASE_URL_ALIAS)) {
            if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
                return Err(ConfigError::DatabaseUrl);
            }
            config.database_url = Some(url);
        }
        // Parsed so an existing environment still boots. Registration is the
        // instance setting in the database, not this flag. GLIMWATCH_OPEN_SIGNUP
        // is ignored.
        if let Some(signup) = optional(OPEN_SIGNUP) {
            config.open_signup = parse_bool(&signup).ok_or(ConfigError::Invalid {
                name: OPEN_SIGNUP,
                value: signup,
            })?;
        }
        if let Some(host) = optional(SMTP_HOST) {
            let from = optional(SMTP_FROM).ok_or(ConfigError::Missing(SMTP_FROM))?;
            let port = match optional(SMTP_PORT) {
                Some(value) => value.parse().map_err(|_| ConfigError::Invalid {
                    name: SMTP_PORT,
                    value,
                })?,
                None => 587,
            };
            config.smtp = Some(SmtpConfig {
                host,
                port,
                username: optional(SMTP_USERNAME),
                password: optional(SMTP_PASSWORD),
                from,
            });
        }
        if let Some(memory) = optional(ARGON_MEMORY) {
            config.argon_memory_kib = memory.parse().map_err(|_| ConfigError::Invalid {
                name: ARGON_MEMORY,
                value: memory,
            })?;
        }
        if let Some(targets) = optional(CONNECTIVITY_TARGETS) {
            config.connectivity_targets = parse_targets(&targets).ok_or(ConfigError::Invalid {
                name: CONNECTIVITY_TARGETS,
                value: targets,
            })?;
        }
        if let Some(hops) = optional(PROXY_HOPS) {
            config.proxy_hops = hops.parse().map_err(|_| ConfigError::Invalid {
                name: PROXY_HOPS,
                value: hops,
            })?;
        }
        if let Some(path) = optional(GEOIP_PATH) {
            config.geoip_path = Some(PathBuf::from(path));
        }
        Ok(config)
    }

    pub fn apply_listen(&mut self, listen: SocketAddr) {
        self.bind = listen.ip();
        self.port = listen.port();
    }

    #[must_use]
    pub const fn listen_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind, self.port)
    }

    #[must_use]
    pub fn secure_cookies(&self) -> bool {
        self.public_origin.starts_with("https://")
    }

    #[must_use]
    pub fn backend_name(&self) -> &'static str {
        if self.database_url.is_some() {
            "postgres"
        } else {
            "sqlite"
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtpConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentMode {
    Development,
    Production,
}

impl EnvironmentMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{name} is invalid: {value}")]
    Invalid { name: &'static str, value: String },
    #[error("missing {0}")]
    Missing(&'static str),
    #[error(
        "DATABASE_URL must be a postgres:// or postgresql:// URL. SQLite is the default; set GLIMWATCH_DATABASE_PATH to move the file"
    )]
    DatabaseUrl,
}

fn optional(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

const STOCK_SQLITE_PATHS: &[&str] = &[
    "data/glimwatch.sqlite",
    "/var/lib/glimwatch/glimwatch.sqlite",
];

/// Uses an explicit path as given. The stock defaults keep an existing
/// `status.sqlite` when `glimwatch.sqlite` is not there yet.
fn resolve_sqlite_path(explicit: Option<String>) -> PathBuf {
    let chosen = explicit
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(STOCK_SQLITE_PATHS[0]));
    if chosen.exists()
        || explicit
            .as_deref()
            .is_some_and(|path| !is_stock_sqlite_path(path))
    {
        return chosen;
    }
    legacy_sqlite_candidates(&chosen)
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or(chosen)
}

fn is_stock_sqlite_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/");
    STOCK_SQLITE_PATHS.contains(&normalized.as_str())
}

fn legacy_sqlite_candidates(chosen: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(parent) = chosen.parent()
        && !parent.as_os_str().is_empty()
    {
        paths.push(parent.join("status.sqlite"));
    }
    for candidate in ["data/status.sqlite", "/var/lib/glimwatch/status.sqlite"] {
        paths.push(PathBuf::from(candidate));
    }
    let mut seen = Vec::new();
    for path in paths {
        if path != chosen && !seen.contains(&path) {
            seen.push(path);
        }
    }
    seen
}

fn parse_targets(value: &str) -> Option<Vec<String>> {
    if value == "off" {
        return Some(Vec::new());
    }
    value
        .split(',')
        .map(str::trim)
        .map(|target| {
            let (host, port) = target.rsplit_once(':')?;
            (!host.is_empty() && port.parse::<u16>().is_ok()).then(|| target.to_owned())
        })
        .collect()
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// `url` with its password replaced, for logs.
#[must_use]
pub fn redact_database_url(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(mut parsed) => {
            if parsed.password().is_some() {
                let _ = parsed.set_password(Some("REDACTED"));
            }
            parsed.into()
        }
        Err(_) => "[invalid database URL]".to_owned(),
    }
}

impl fmt::Display for Config {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let database = self
            .database_url
            .as_deref()
            .map(redact_database_url)
            .unwrap_or_else(|| self.database_path.display().to_string());
        write!(
            formatter,
            "env={} listen={} database={database} signup={}",
            self.environment.as_str(),
            self.listen_addr(),
            if self.open_signup { "open" } else { "closed" }
        )
    }
}

impl FromStr for EnvironmentMode {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "development" => Ok(Self::Development),
            "production" => Ok(Self::Production),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;

    #[test]
    fn parses_connectivity_targets() {
        assert_eq!(
            parse_targets("1.1.1.1:443, [::1]:53"),
            Some(vec!["1.1.1.1:443".to_owned(), "[::1]:53".to_owned()])
        );
        assert_eq!(parse_targets("off"), Some(Vec::new()));
        assert_eq!(parse_targets("1.1.1.1"), None);
        assert_eq!(parse_targets("1.1.1.1:443,"), None);
    }

    #[test]
    fn redacts_passwords() {
        assert_eq!(
            redact_database_url("postgres://status:secret@localhost/status"),
            "postgres://status:REDACTED@localhost/status"
        );
    }

    /// `Config::from_env` reads process globals. These two tests must not overlap.
    fn lock_process_env() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        LOCK.lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    #[test]
    fn rejects_sqlite_database_urls() {
        let _env = lock_process_env();
        // SAFETY: the lock above is the only other writer of these variables.
        unsafe {
            std::env::set_var("DATABASE_URL", "sqlite://data/ignored.sqlite");
            std::env::remove_var("GLIMWATCH_DATABASE_URL");
        }
        let error = Config::from_env().unwrap_err();
        assert!(matches!(error, ConfigError::DatabaseUrl));
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
    }

    #[test]
    fn status_database_url_is_ignored() {
        let _env = lock_process_env();
        unsafe {
            std::env::remove_var("GLIMWATCH_DATABASE_URL");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("STATUS_DATABASE_URL");
            // STATUS_* is not an alias. This URL must not select Postgres.
            std::env::set_var(
                "STATUS_DATABASE_URL",
                "postgres://status:secret@127.0.0.1:1/status",
            );
        }
        let ignored = Config::from_env().unwrap();
        assert_eq!(ignored.database_url.as_deref(), None);
        unsafe {
            std::env::set_var("DATABASE_URL", "postgres://alias:secret@127.0.0.1:1/alias");
        }
        let alias = Config::from_env().unwrap();
        assert_eq!(
            alias.database_url.as_deref(),
            Some("postgres://alias:secret@127.0.0.1:1/alias")
        );
        unsafe {
            std::env::set_var(
                "GLIMWATCH_DATABASE_URL",
                "postgres://glimwatch:secret@127.0.0.1:1/glimwatch",
            );
        }
        let preferred = Config::from_env().unwrap();
        assert_eq!(
            preferred.database_url.as_deref(),
            Some("postgres://glimwatch:secret@127.0.0.1:1/glimwatch")
        );
        unsafe {
            std::env::remove_var("GLIMWATCH_DATABASE_URL");
            std::env::remove_var("DATABASE_URL");
            std::env::remove_var("STATUS_DATABASE_URL");
        }
    }

    #[test]
    fn stock_sqlite_paths_prefer_older_files_and_custom_paths_stay() {
        assert!(is_stock_sqlite_path("data/glimwatch.sqlite"));
        assert!(is_stock_sqlite_path("/var/lib/glimwatch/glimwatch.sqlite"));
        assert!(!is_stock_sqlite_path("/tmp/custom.sqlite"));
        assert!(!is_stock_sqlite_path("data/status.sqlite"));
        let candidates = legacy_sqlite_candidates(Path::new("data/glimwatch.sqlite"));
        assert_eq!(
            candidates,
            [
                PathBuf::from("data/status.sqlite"),
                PathBuf::from("/var/lib/glimwatch/status.sqlite"),
            ]
        );
        let custom = std::env::temp_dir().join(format!(
            "glimwatch-missing-{}-{}.sqlite",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert_eq!(
            resolve_sqlite_path(Some(custom.display().to_string())),
            custom
        );
    }
}
