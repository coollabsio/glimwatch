//! Shared platform for Owlee.
//!
//! This crate mirrors the role of Orbit's `orbit-platform`: configuration,
//! the SQLite/Postgres access layer, migrations, password hashing, identifiers,
//! timestamps, and RFC 9457 problem responses. Owlee does not include
//! Orbit's realtime, file, or collaboration subsystems.

mod auth;
mod config;
mod db;
mod id;
mod observability;
mod problem;
mod time;

pub use auth::{PasswordError, PasswordService, generate_token, hash_token, normalize_email};
pub use config::{Config, ConfigError, EnvironmentMode, SmtpConfig, redact_database_url};
pub use db::{
    Backend, Database, DatabaseConfig, DatabaseError, Migration, MigrationError, MigrationRunner,
};
pub use id::{Id, ParseIdError};
pub use observability::{TracingFormat, init_tracing};
pub use problem::Problem;
pub use time::TimestampMillis;
