use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use thiserror::Error;

const ENV: &str = "STATUS_ENV";
const BIND: &str = "STATUS_BIND";
const PORT: &str = "STATUS_PORT";
const ORIGIN: &str = "STATUS_PUBLIC_ORIGIN";
const DATABASE_PATH: &str = "STATUS_DATABASE_PATH";
const DATABASE_URL: &str = "STATUS_DATABASE_URL";
const DATABASE_URL_ALIAS: &str = "DATABASE_URL";
const OPEN_SIGNUP: &str = "STATUS_OPEN_SIGNUP";
const SMTP_HOST: &str = "STATUS_SMTP_HOST";
const SMTP_PORT: &str = "STATUS_SMTP_PORT";
const SMTP_USERNAME: &str = "STATUS_SMTP_USERNAME";
const SMTP_PASSWORD: &str = "STATUS_SMTP_PASSWORD";
const SMTP_FROM: &str = "STATUS_SMTP_FROM";
const ARGON_MEMORY: &str = "STATUS_ARGON_MEMORY_KIB";
const CONNECTIVITY_TARGETS: &str = "STATUS_CONNECTIVITY_TARGETS";

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
    /// `host:port` targets that prove the Status host itself has network.
    /// Empty disables the check.
    pub connectivity_targets: Vec<String>,
}

impl Config {
    #[must_use]
    pub fn development() -> Self {
        Self {
            environment: EnvironmentMode::Development,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 43123,
            public_origin: "http://127.0.0.1:43123".to_owned(),
            database_path: PathBuf::from("data/status.sqlite"),
            database_url: None,
            open_signup: true,
            smtp: None,
            argon_memory_kib: 19456,
            connectivity_targets: ["1.1.1.1:443", "8.8.8.8:443", "9.9.9.9:443"]
                .map(str::to_owned)
                .to_vec(),
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
        if let Some(path) = optional(DATABASE_PATH) {
            config.database_path = PathBuf::from(path);
        }
        if let Some(url) = optional(DATABASE_URL).or_else(|| optional(DATABASE_URL_ALIAS)) {
            if !(url.starts_with("postgres://") || url.starts_with("postgresql://")) {
                return Err(ConfigError::DatabaseUrl);
            }
            config.database_url = Some(url);
        }
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
        "DATABASE_URL must be a postgres:// or postgresql:// URL. SQLite is the default; set STATUS_DATABASE_PATH to move the file"
    )]
    DatabaseUrl,
}

fn optional(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
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

    #[test]
    fn rejects_sqlite_database_urls() {
        // SAFETY: this test is the only writer of the variable, and it restores nothing
        // because the process is the test binary. Other tests do not read DATABASE_URL.
        unsafe {
            std::env::set_var("DATABASE_URL", "sqlite://data/ignored.sqlite");
            std::env::remove_var("STATUS_DATABASE_URL");
        }
        let error = Config::from_env().unwrap_err();
        assert!(matches!(error, ConfigError::DatabaseUrl));
        unsafe {
            std::env::remove_var("DATABASE_URL");
        }
    }
}
