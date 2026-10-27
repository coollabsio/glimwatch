//! Settings of this Haruvo instance that the root user changes.
//!
//! Registration stays closed after setup. Opening it requires a mail server, either
//! saved here or set with `HARUVO_SMTP_*`. Clearing the saved server closes registration.
//! The environment variables are not cleared by that action; they still count as a
//! mail server until they are unset.

use std::ops::DerefMut;

use haruvo_platform::{Database, Id, ParseIdError, SmtpConfig, TimestampMillis};
use sqlx::{Any, Row, Transaction};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmtpSettings {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

impl From<SmtpSettings> for SmtpConfig {
    fn from(smtp: SmtpSettings) -> Self {
        Self {
            host: smtp.host,
            port: smtp.port,
            username: smtp.username,
            password: smtp.password,
            from: smtp.from,
        }
    }
}

/// Mail server fields from Settings. `password: None` keeps the stored password.
pub struct SmtpInput {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub from: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RootUser {
    pub id: Id,
    pub email: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstanceView {
    pub registration_open: bool,
    pub smtp: Option<SmtpSettings>,
    pub initialized: bool,
    pub root_user: Option<RootUser>,
}

impl InstanceView {
    /// Public sign-up is on only after setup, and only when the root user opened it
    /// and a mail server is available.
    #[must_use]
    pub fn signup_open(&self, env_smtp: bool, setup_complete: bool) -> bool {
        setup_complete && self.registration_open && self.smtp_ready(env_smtp)
    }

    #[must_use]
    pub fn smtp_ready(&self, env_smtp: bool) -> bool {
        self.smtp.is_some() || env_smtp
    }
}

#[derive(Debug, Error)]
pub enum InstanceError {
    #[error("registration needs a configured mail server")]
    EmailNotConfigured,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

pub async fn load(db: &Database) -> Result<InstanceView, sqlx::Error> {
    ensure_row(db).await?;
    let row = sqlx::query(
        "SELECT registration_open, smtp_host, smtp_port, smtp_username, smtp_password, smtp_from \
         FROM instance_settings WHERE id = 1",
    )
    .fetch_one(db.pool())
    .await?;
    let registration_open: i64 = row.try_get("registration_open")?;
    let smtp = smtp_from_row(
        row.try_get("smtp_host")?,
        row.try_get("smtp_port")?,
        row.try_get("smtp_username")?,
        row.try_get("smtp_password")?,
        row.try_get("smtp_from")?,
    );
    let state = sqlx::query(
        "SELECT s.initialized, s.root_user_id, u.email, u.display_name \
         FROM installation_state s \
         LEFT JOIN users u ON u.id = s.root_user_id \
         WHERE s.id = 1",
    )
    .fetch_optional(db.pool())
    .await?;
    let (initialized, root_user) = match state {
        Some(state) => {
            let initialized: i64 = state.try_get("initialized")?;
            let root_user = match (
                optional_id(&state, "root_user_id")?,
                state.try_get::<Option<String>, _>("email")?,
                state.try_get::<Option<String>, _>("display_name")?,
            ) {
                (Some(id), Some(email), Some(display_name)) => Some(RootUser {
                    id,
                    email,
                    display_name,
                }),
                _ => None,
            };
            (initialized != 0, root_user)
        }
        None => (false, None),
    };
    Ok(InstanceView {
        registration_open: registration_open != 0,
        smtp,
        initialized,
        root_user,
    })
}

pub async fn root_user_id(db: &Database) -> Result<Option<Id>, sqlx::Error> {
    let row = sqlx::query("SELECT root_user_id FROM installation_state WHERE id = 1")
        .fetch_optional(db.pool())
        .await?;
    row.as_ref()
        .map(|row| optional_id(row, "root_user_id"))
        .transpose()
        .map(Option::flatten)
}

/// Opening registration needs a mail server. Closing does not.
pub async fn set_registration_open(
    db: &Database,
    open: bool,
    env_smtp: bool,
    now: TimestampMillis,
) -> Result<(), InstanceError> {
    let mut tx = db.begin_immediate().await?;
    ensure_row_tx(&mut tx).await?;
    if open {
        let row = sqlx::query("SELECT smtp_host, smtp_from FROM instance_settings WHERE id = 1")
            .fetch_one(tx.deref_mut())
            .await?;
        let host: Option<String> = row.try_get("smtp_host")?;
        let from: Option<String> = row.try_get("smtp_from")?;
        let saved =
            host.is_some_and(|host| !host.is_empty()) && from.is_some_and(|from| !from.is_empty());
        if !saved && !env_smtp {
            return Err(InstanceError::EmailNotConfigured);
        }
    }
    sqlx::query(
        "UPDATE instance_settings SET registration_open = $1, updated_at = $2 WHERE id = 1",
    )
    .bind(i64::from(open))
    .bind(now.as_millis())
    .execute(tx.deref_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn save_smtp(
    db: &Database,
    input: SmtpInput,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin_immediate().await?;
    ensure_row_tx(&mut tx).await?;
    let password = match input.password {
        Some(password) => Some(password),
        None => {
            let row = sqlx::query("SELECT smtp_password FROM instance_settings WHERE id = 1")
                .fetch_one(tx.deref_mut())
                .await?;
            blank_to_none(row.try_get("smtp_password")?)
        }
    };
    sqlx::query(
        "UPDATE instance_settings SET smtp_host = $1, smtp_port = $2, smtp_username = $3, \
         smtp_password = $4, smtp_from = $5, updated_at = $6 WHERE id = 1",
    )
    .bind(&input.host)
    .bind(i64::from(input.port))
    .bind(&input.username)
    .bind(&password)
    .bind(&input.from)
    .bind(now.as_millis())
    .execute(tx.deref_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Removes the saved mail server and closes registration.
/// Environment `HARUVO_SMTP_*` is left as it was.
pub async fn clear_smtp(db: &Database, now: TimestampMillis) -> Result<(), sqlx::Error> {
    let mut tx = db.begin_immediate().await?;
    ensure_row_tx(&mut tx).await?;
    sqlx::query(
        "UPDATE instance_settings SET smtp_host = NULL, smtp_port = NULL, smtp_username = NULL, \
         smtp_password = NULL, smtp_from = NULL, registration_open = 0, updated_at = $1 WHERE id = 1",
    )
    .bind(now.as_millis())
    .execute(tx.deref_mut())
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Opens registration and stores a mail server. Shared Postgres tests call this when
/// setup already ran on a reused database and they still need to create an account.
pub async fn permit_signup(db: &Database) -> Result<(), sqlx::Error> {
    let now = TimestampMillis::now();
    save_smtp(
        db,
        SmtpInput {
            host: "smtp.example.com".to_owned(),
            port: 587,
            username: None,
            password: None,
            from: "status@example.com".to_owned(),
        },
        now,
    )
    .await?;
    let mut tx = db.begin_immediate().await?;
    sqlx::query("UPDATE instance_settings SET registration_open = 1, updated_at = $1 WHERE id = 1")
        .bind(now.as_millis())
        .execute(tx.deref_mut())
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn ensure_row(db: &Database) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO instance_settings (id, registration_open) VALUES (1, 0) ON CONFLICT DO NOTHING",
    )
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn ensure_row_tx(tx: &mut Transaction<'_, Any>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO instance_settings (id, registration_open) VALUES (1, 0) ON CONFLICT DO NOTHING",
    )
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

fn smtp_from_row(
    host: Option<String>,
    port: Option<i64>,
    username: Option<String>,
    password: Option<String>,
    from: Option<String>,
) -> Option<SmtpSettings> {
    let host = host.filter(|host| !host.is_empty())?;
    let from = from.filter(|from| !from.is_empty())?;
    let port = u16::try_from(port?).ok().filter(|port| *port != 0)?;
    Some(SmtpSettings {
        host,
        port,
        username: blank_to_none(username),
        password: blank_to_none(password),
        from,
    })
}

fn blank_to_none(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

fn optional_id(row: &sqlx::any::AnyRow, name: &str) -> Result<Option<Id>, sqlx::Error> {
    let value: Option<String> = row.try_get(name)?;
    value
        .map(|value| {
            value.parse().map_err(|error: ParseIdError| {
                sqlx::Error::Decode(Box::new(std::io::Error::other(error.to_string())))
            })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(open: bool, smtp: bool) -> InstanceView {
        InstanceView {
            registration_open: open,
            smtp: smtp.then(|| SmtpSettings {
                host: "smtp.example.com".to_owned(),
                port: 587,
                username: None,
                password: None,
                from: "status@example.com".to_owned(),
            }),
            initialized: true,
            root_user: None,
        }
    }

    #[test]
    fn signup_needs_setup_the_flag_and_a_mail_server() {
        assert!(!view(true, true).signup_open(false, false));
        assert!(!view(false, true).signup_open(false, true));
        assert!(!view(true, false).signup_open(false, true));
        assert!(view(true, false).signup_open(true, true));
        assert!(view(true, true).signup_open(false, true));
    }
}
