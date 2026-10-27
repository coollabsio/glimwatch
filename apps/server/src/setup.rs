//! One-time installation setup, adapted from Orbit's setup token.
//!
//! A fresh database gets a single opaque token (hash stored, 30 minute lifetime).
//! Completing setup creates the owner, marks `installation_state` initialized, and
//! deletes every setup token. Setup cannot run again. Public registration stays closed
//! unless `HARUVO_OPEN_SIGNUP` is set after that.

use std::ops::DerefMut;

use haruvo_platform::{Database, Id, TimestampMillis, generate_token, hash_token};
use sqlx::{Any, Row, Transaction};
use thiserror::Error;

use crate::repo::{self, User};

/// Same lifetime Orbit uses (`IdentityRepository::SETUP_TOKEN_LIFETIME_MILLIS`).
pub const SETUP_TOKEN_LIFETIME_MILLIS: i64 = 30 * 60 * 1_000;

const DAY_MS: i64 = 86_400_000;

#[derive(Debug)]
pub struct IssuedToken {
    pub token: String,
    pub expires_at: TimestampMillis,
}

pub struct NewOwner {
    pub token: String,
    pub email: String,
    pub normalized_email: String,
    pub display_name: String,
    pub password_hash: String,
}

pub struct CompletedSetup {
    pub user: User,
    pub session_token: String,
}

#[derive(Debug, Error)]
pub enum SetupError {
    #[error("setup is already complete and cannot run again")]
    AlreadyComplete,
    #[error("the setup token is invalid or expired")]
    InvalidToken,
    #[error("an account with that email already exists")]
    EmailTaken,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

#[must_use]
pub fn setup_url(origin: &str, token: &str) -> String {
    format!("{}/setup?token={token}", origin.trim_end_matches('/'))
}

pub async fn is_complete(db: &Database) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT COALESCE((SELECT initialized FROM installation_state WHERE id = 1), 0) AS n",
    )
    .fetch_one(db.pool())
    .await?;
    let initialized: i64 = row.try_get("n")?;
    Ok(initialized != 0)
}

/// Issues a setup token only when setup is unfinished and no unexpired token is stored.
/// A second call returns `None` so the server does not print the URL again.
pub async fn initialize_setup_token(
    db: &Database,
    now: TimestampMillis,
) -> Result<Option<IssuedToken>, sqlx::Error> {
    let mut tx = db.begin_immediate().await?;
    ensure_state(&mut tx).await?;
    if read_initialized(&mut tx).await? != 0 {
        return Ok(None);
    }
    sqlx::query("DELETE FROM setup_tokens WHERE expires_at <= $1")
        .bind(now.as_millis())
        .execute(tx.deref_mut())
        .await?;
    if count_tokens(&mut tx).await? != 0 {
        tx.commit().await?;
        return Ok(None);
    }
    let issued = issue(now);
    insert_token(&mut tx, &issued, now).await?;
    tx.commit().await?;
    Ok(Some(issued))
}

/// Replaces every unused setup token. Fails once setup is complete.
pub async fn rotate_setup_token(
    db: &Database,
    now: TimestampMillis,
) -> Result<IssuedToken, SetupError> {
    let mut tx = db.begin_immediate().await?;
    ensure_state(&mut tx).await?;
    if read_initialized(&mut tx).await? != 0 {
        return Err(SetupError::AlreadyComplete);
    }
    sqlx::query("DELETE FROM setup_tokens")
        .execute(tx.deref_mut())
        .await?;
    let issued = issue(now);
    insert_token(&mut tx, &issued, now).await?;
    tx.commit().await?;
    Ok(issued)
}

pub async fn setup_token_valid(
    db: &Database,
    token: &str,
    now: TimestampMillis,
) -> Result<bool, sqlx::Error> {
    if is_complete(db).await? {
        return Ok(false);
    }
    let supplied = hash_token(token);
    let row = sqlx::query("SELECT token_hash, expires_at FROM setup_tokens WHERE token_hash = $1")
        .bind(&supplied)
        .fetch_optional(db.pool())
        .await?;
    Ok(row.is_some_and(|row| token_row_matches(&row, &supplied, now)))
}

pub async fn complete_setup(
    db: &Database,
    owner: NewOwner,
    now: TimestampMillis,
) -> Result<CompletedSetup, SetupError> {
    let mut tx = db.begin_immediate().await?;
    ensure_state(&mut tx).await?;
    lock_state(&mut tx, db).await?;
    if read_initialized(&mut tx).await? != 0 {
        return Err(SetupError::AlreadyComplete);
    }
    let supplied = hash_token(&owner.token);
    let row = sqlx::query("SELECT token_hash, expires_at FROM setup_tokens WHERE token_hash = $1")
        .bind(&supplied)
        .fetch_optional(tx.deref_mut())
        .await?;
    let valid = row.is_some_and(|row| token_row_matches(&row, &supplied, now));
    if !valid {
        return Err(SetupError::InvalidToken);
    }
    let taken = sqlx::query("SELECT COUNT(*) AS n FROM users WHERE normalized_email = $1")
        .bind(&owner.normalized_email)
        .fetch_one(tx.deref_mut())
        .await?;
    let taken: i64 = taken.try_get("n")?;
    if taken != 0 {
        return Err(SetupError::EmailTaken);
    }

    let user_id = Id::new();
    let session_id = Id::new();
    let session_token = generate_token();
    let timestamp = now.as_millis();
    sqlx::query(
        "INSERT INTO users (id, email, normalized_email, display_name, password_hash, role, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, 'owner', $6, $6)",
    )
    .bind(user_id.to_string())
    .bind(&owner.email)
    .bind(&owner.normalized_email)
    .bind(&owner.display_name)
    .bind(&owner.password_hash)
    .bind(timestamp)
    .execute(tx.deref_mut())
    .await?;
    let idle = TimestampMillis::from_millis(timestamp + 30 * DAY_MS);
    let absolute = TimestampMillis::from_millis(timestamp + 90 * DAY_MS);
    sqlx::query(
        "INSERT INTO sessions (id, token_hash, user_id, created_at, last_activity_at, idle_expires_at, absolute_expires_at) \
         VALUES ($1, $2, $3, $4, $4, $5, $6)",
    )
    .bind(session_id.to_string())
    .bind(hash_token(&session_token))
    .bind(user_id.to_string())
    .bind(timestamp)
    .bind(idle.as_millis())
    .bind(absolute.as_millis())
    .execute(tx.deref_mut())
    .await?;
    let updated = sqlx::query(
        "UPDATE installation_state SET initialized = 1, initialized_at = $1, root_user_id = $2 \
         WHERE id = 1 AND initialized = 0",
    )
    .bind(timestamp)
    .bind(user_id.to_string())
    .execute(tx.deref_mut())
    .await?;
    if updated.rows_affected() != 1 {
        return Err(SetupError::AlreadyComplete);
    }
    sqlx::query("DELETE FROM setup_tokens")
        .execute(tx.deref_mut())
        .await?;
    sqlx::query(
        "INSERT INTO audit_events (id, actor_id, action, resource_type, resource_id, metadata_json, occurred_at) \
         VALUES ($1, $2, 'setup.complete', 'installation', $3, '{}', $4)",
    )
    .bind(Id::new().to_string())
    .bind(user_id.to_string())
    .bind(user_id.to_string())
    .bind(timestamp)
    .execute(tx.deref_mut())
    .await?;
    tx.commit().await?;
    let user = repo::user_by_email(db, &owner.normalized_email)
        .await?
        .ok_or(sqlx::Error::RowNotFound)?;
    Ok(CompletedSetup {
        user,
        session_token,
    })
}

/// Marks setup finished without a token. Local `seed` uses this for the demo owner.
pub async fn mark_initialized(
    db: &Database,
    root_user_id: Id,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    let mut tx = db.begin_immediate().await?;
    ensure_state(&mut tx).await?;
    sqlx::query(
        "UPDATE installation_state SET initialized = 1, initialized_at = $1, root_user_id = $2 WHERE id = 1",
    )
    .bind(now.as_millis())
    .bind(root_user_id.to_string())
    .execute(tx.deref_mut())
    .await?;
    sqlx::query("DELETE FROM setup_tokens")
        .execute(tx.deref_mut())
        .await?;
    tx.commit().await?;
    Ok(())
}

async fn ensure_state(tx: &mut Transaction<'_, Any>) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO installation_state (id, initialized) VALUES (1, 0) ON CONFLICT DO NOTHING",
    )
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

async fn lock_state(tx: &mut Transaction<'_, Any>, db: &Database) -> Result<(), sqlx::Error> {
    if db.backend() == haruvo_platform::Backend::Postgres {
        sqlx::query("SELECT initialized FROM installation_state WHERE id = 1 FOR UPDATE")
            .fetch_one(tx.deref_mut())
            .await?;
    }
    Ok(())
}

async fn read_initialized(tx: &mut Transaction<'_, Any>) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT initialized FROM installation_state WHERE id = 1")
        .fetch_one(tx.deref_mut())
        .await?;
    row.try_get("initialized")
}

async fn count_tokens(tx: &mut Transaction<'_, Any>) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM setup_tokens")
        .fetch_one(tx.deref_mut())
        .await?;
    row.try_get("n")
}

fn issue(now: TimestampMillis) -> IssuedToken {
    IssuedToken {
        token: generate_token(),
        expires_at: TimestampMillis::from_millis(now.as_millis() + SETUP_TOKEN_LIFETIME_MILLIS),
    }
}

async fn insert_token(
    tx: &mut Transaction<'_, Any>,
    issued: &IssuedToken,
    now: TimestampMillis,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO setup_tokens (token_hash, expires_at, created_at) VALUES ($1, $2, $3)",
    )
    .bind(hash_token(&issued.token))
    .bind(issued.expires_at.as_millis())
    .bind(now.as_millis())
    .execute(tx.deref_mut())
    .await?;
    Ok(())
}

fn token_row_matches(row: &sqlx::any::AnyRow, supplied: &str, now: TimestampMillis) -> bool {
    let hash: String = row.try_get("token_hash").unwrap_or_default();
    let expires_at: i64 = row.try_get("expires_at").unwrap_or(0);
    expires_at > now.as_millis() && constant_time_eq(hash.as_bytes(), supplied.as_bytes())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right) {
        diff |= a ^ b;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use sqlx::Row;

    use super::*;

    #[tokio::test]
    async fn only_the_hash_is_stored_and_an_expired_token_can_be_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let state = crate::state::boot(crate::state::testing_config(
            dir.path().join("setup.sqlite"),
        ))
        .await
        .unwrap();
        let now = TimestampMillis::now();
        let issued = initialize_setup_token(&state.db, now)
            .await
            .unwrap()
            .unwrap();
        let stored: String = sqlx::query("SELECT token_hash AS n FROM setup_tokens")
            .fetch_one(state.db.pool())
            .await
            .unwrap()
            .try_get("n")
            .unwrap();
        assert_ne!(stored, issued.token);
        assert!(!stored.contains(&issued.token));
        assert!(!setup_token_valid(&state.db, "wrong", now).await.unwrap());
        assert!(
            setup_token_valid(&state.db, &issued.token, now)
                .await
                .unwrap()
        );

        sqlx::query("UPDATE setup_tokens SET expires_at = 1")
            .execute(state.db.pool())
            .await
            .unwrap();
        assert!(
            !setup_token_valid(&state.db, &issued.token, now)
                .await
                .unwrap()
        );
        let replaced = initialize_setup_token(&state.db, now)
            .await
            .unwrap()
            .expect("expired token is replaced");
        assert_ne!(replaced.token, issued.token);
        assert!(
            setup_token_valid(&state.db, &replaced.token, now)
                .await
                .unwrap()
        );
    }
}
