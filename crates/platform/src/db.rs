use std::path::{Path, PathBuf};
use std::sync::Once;
use std::time::Duration;

use sha2::{Digest, Sha256};
use sqlx::any::AnyPoolOptions;
use sqlx::{Any, AnyPool, Row};
use thiserror::Error;

use crate::auth::hex_encode;
use crate::time::TimestampMillis;

/// Which engine an open [`Database`] is speaking. Queries in Status are
/// written once and run on both; integer columns are `BIGINT` so sqlx's Any
/// driver decodes them as `i64` on SQLite and Postgres.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backend {
    Sqlite,
    Postgres,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub path: PathBuf,
    pub postgres_url: Option<String>,
    pub max_connections: u32,
}

impl DatabaseConfig {
    #[must_use]
    pub fn sqlite(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            postgres_url: None,
            max_connections: 8,
        }
    }

    #[must_use]
    pub fn postgres(url: impl Into<String>) -> Self {
        Self {
            path: PathBuf::from("data/status.sqlite"),
            postgres_url: Some(url.into()),
            max_connections: 16,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Database {
    pool: AnyPool,
    backend: Backend,
}

impl Database {
    pub async fn connect(config: &DatabaseConfig) -> Result<Self, DatabaseError> {
        install_drivers();
        if let Some(url) = &config.postgres_url {
            let pool = AnyPoolOptions::new()
                .max_connections(config.max_connections)
                .acquire_timeout(Duration::from_secs(10))
                .connect(url)
                .await?;
            return Ok(Self {
                pool,
                backend: Backend::Postgres,
            });
        }

        if let Some(parent) = config.path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(DatabaseError::CreateDir)?;
        }
        let url = sqlite_url(&config.path);
        let pool = AnyPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .after_connect(|connection, _metadata| {
                Box::pin(async move {
                    sqlx::query("PRAGMA foreign_keys = ON")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("PRAGMA journal_mode = WAL")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("PRAGMA busy_timeout = 5000")
                        .execute(&mut *connection)
                        .await?;
                    sqlx::query("PRAGMA synchronous = NORMAL")
                        .execute(&mut *connection)
                        .await?;
                    Ok(())
                })
            })
            .connect(&url)
            .await?;
        Ok(Self {
            pool,
            backend: Backend::Sqlite,
        })
    }

    #[must_use]
    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    #[must_use]
    pub const fn backend(&self) -> Backend {
        self.backend
    }

    pub async fn begin(&self) -> Result<sqlx::Transaction<'_, Any>, sqlx::Error> {
        self.pool.begin().await
    }
}

fn install_drivers() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        sqlx::any::install_default_drivers();
    });
}

fn sqlite_url(path: &Path) -> String {
    format!("sqlite://{}?mode=rwc", path.display())
}

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("failed to create the database directory: {0}")]
    CreateDir(std::io::Error),
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

#[derive(Clone, Debug)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    #[must_use]
    pub const fn new(version: i64, name: &'static str, sql: &'static str) -> Self {
        Self { version, name, sql }
    }

    #[must_use]
    pub fn checksum(&self) -> String {
        hex_encode(Sha256::digest(self.sql.as_bytes()).as_slice())
    }
}

#[derive(Debug)]
pub struct MigrationRunner {
    app_version: String,
    migrations: Vec<Migration>,
}

impl MigrationRunner {
    #[must_use]
    pub fn new(app_version: impl Into<String>, mut migrations: Vec<Migration>) -> Self {
        migrations.sort_by_key(|migration| migration.version);
        Self {
            app_version: app_version.into(),
            migrations,
        }
    }

    pub async fn run(&self, database: &Database) -> Result<usize, MigrationError> {
        self.validate_catalog()?;
        self.ensure_metadata(database).await?;
        let applied = self.applied(database).await?;
        let pending: Vec<&Migration> = self
            .migrations
            .iter()
            .filter(|migration| !applied.iter().any(|version| *version == migration.version))
            .collect();
        let count = pending.len();
        for migration in pending {
            self.apply(database, migration).await?;
        }
        Ok(count)
    }

    fn validate_catalog(&self) -> Result<(), MigrationError> {
        for pair in self.migrations.windows(2) {
            if pair[0].version == pair[1].version {
                return Err(MigrationError::DuplicateVersion {
                    version: pair[0].version,
                });
            }
        }
        Ok(())
    }

    async fn ensure_metadata(&self, database: &Database) -> Result<(), MigrationError> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS schema_migrations (\
                version BIGINT PRIMARY KEY,\
                name TEXT NOT NULL,\
                checksum TEXT NOT NULL,\
                applied_at BIGINT NOT NULL,\
                app_version TEXT NOT NULL\
            )",
        )
        .execute(database.pool())
        .await?;
        Ok(())
    }

    async fn applied(&self, database: &Database) -> Result<Vec<i64>, MigrationError> {
        let rows = sqlx::query("SELECT version, checksum FROM schema_migrations ORDER BY version")
            .fetch_all(database.pool())
            .await?;
        let binary_version = self
            .migrations
            .last()
            .map_or(0, |migration| migration.version);
        let database_version = rows.last().map_or(0, |row| row.get::<i64, _>("version"));
        if database_version > binary_version {
            return Err(MigrationError::SchemaNewer {
                database_version,
                binary_version,
            });
        }
        let mut applied = Vec::with_capacity(rows.len());
        for row in rows {
            let version = row.try_get::<i64, _>("version")?;
            let actual = row.try_get::<String, _>("checksum")?;
            let migration = self
                .migrations
                .iter()
                .find(|migration| migration.version == version)
                .ok_or(MigrationError::Unknown { version })?;
            let expected = migration.checksum();
            if actual != expected {
                return Err(MigrationError::Checksum {
                    version,
                    expected,
                    actual,
                });
            }
            applied.push(version);
        }
        Ok(applied)
    }

    async fn apply(
        &self,
        database: &Database,
        migration: &Migration,
    ) -> Result<(), MigrationError> {
        let mut transaction = database.begin().await?;
        for statement in statements(migration.sql) {
            sqlx::query(&statement)
                .execute(&mut *transaction)
                .await
                .map_err(|source| MigrationError::Apply {
                    version: migration.version,
                    name: migration.name,
                    source,
                })?;
        }
        sqlx::query(
            "INSERT INTO schema_migrations (version, name, checksum, applied_at, app_version) \
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(migration.version)
        .bind(migration.name)
        .bind(migration.checksum())
        .bind(TimestampMillis::now().as_millis())
        .bind(&self.app_version)
        .execute(&mut *transaction)
        .await
        .map_err(|source| MigrationError::Apply {
            version: migration.version,
            name: migration.name,
            source,
        })?;
        transaction
            .commit()
            .await
            .map_err(|source| MigrationError::Apply {
                version: migration.version,
                name: migration.name,
                source,
            })?;
        tracing::info!(
            version = migration.version,
            name = migration.name,
            backend = ?database.backend(),
            "applied migration"
        );
        Ok(())
    }
}

/// Splits a migration into statements. Status migrations keep semicolons
/// at the end of statements only, and use `--` for full-line comments.
pub fn statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("--") {
            continue;
        }
        current.push_str(line);
        current.push('\n');
        if trimmed.ends_with(';') {
            push_statement(&mut statements, &mut current);
        }
    }
    push_statement(&mut statements, &mut current);
    statements
}

fn push_statement(statements: &mut Vec<String>, current: &mut String) {
    let statement = current.trim().trim_end_matches(';').trim().to_owned();
    if !statement.is_empty() {
        statements.push(statement);
    }
    current.clear();
}

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("migration version {version} appears more than once")]
    DuplicateVersion { version: i64 },
    #[error(
        "database schema version {database_version} is newer than this binary ({binary_version})"
    )]
    SchemaNewer {
        database_version: i64,
        binary_version: i64,
    },
    #[error("migration {version} is not known to this binary")]
    Unknown { version: i64 },
    #[error(
        "migration {version} checksum mismatch: expected {expected}, found {actual}. The database was migrated by different SQL than this binary"
    )]
    Checksum {
        version: i64,
        expected: String,
        actual: String,
    },
    #[error("migration {version} ({name}) failed: {source}")]
    Apply {
        version: i64,
        name: &'static str,
        source: sqlx::Error,
    },
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDGETS: &str = "\
CREATE TABLE widgets (
    id TEXT PRIMARY KEY,
    n BIGINT NOT NULL
);
";

    async fn assert_round_trip(database: &Database) {
        let runner = MigrationRunner::new("test", vec![Migration::new(1, "widgets", WIDGETS)]);
        assert_eq!(runner.run(database).await.unwrap(), 1);
        assert_eq!(runner.run(database).await.unwrap(), 0);
        sqlx::query("INSERT INTO widgets (id, n) VALUES ($1, $2)")
            .bind("a")
            .bind(7_i64)
            .execute(database.pool())
            .await
            .unwrap();
        let row = sqlx::query("SELECT n FROM widgets WHERE id = $1")
            .bind("a")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(row.get::<i64, _>("n"), 7);
    }

    #[tokio::test]
    async fn sqlite_migrations_are_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::connect(&DatabaseConfig::sqlite(dir.path().join("t.sqlite")))
            .await
            .unwrap();
        assert_eq!(database.backend(), Backend::Sqlite);
        assert_round_trip(&database).await;
    }

    #[tokio::test]
    async fn postgres_migrations_when_configured() {
        let url = match std::env::var("STATUS_TEST_POSTGRES_URL") {
            Ok(url) if url.starts_with("postgres") => url,
            _ => return,
        };
        let database = Database::connect(&DatabaseConfig::postgres(url))
            .await
            .unwrap();
        assert_eq!(database.backend(), Backend::Postgres);
        // Isolate the platform fixture from the application schema.
        sqlx::query("DROP TABLE IF EXISTS widgets")
            .execute(database.pool())
            .await
            .unwrap();
        sqlx::query("DROP TABLE IF EXISTS schema_migrations")
            .execute(database.pool())
            .await
            .unwrap();
        assert_round_trip(&database).await;
    }

    #[test]
    fn splits_comments_and_statements() {
        let sql = "-- header\nCREATE TABLE a (id TEXT);\n\n-- next\nCREATE TABLE b (id TEXT);\n";
        assert_eq!(
            statements(sql),
            vec!["CREATE TABLE a (id TEXT)", "CREATE TABLE b (id TEXT)"]
        );
    }
}
