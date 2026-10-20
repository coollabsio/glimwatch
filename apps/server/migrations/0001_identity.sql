-- Accounts, sessions, and the single-runner scheduler lease.
-- Integer columns are BIGINT so the same SQL decodes as i64 on SQLite and Postgres.

CREATE TABLE users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL,
    normalized_email TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role TEXT NOT NULL CHECK (role IN ('owner', 'admin', 'member')),
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    token_hash TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    last_activity_at BIGINT NOT NULL,
    idle_expires_at BIGINT NOT NULL,
    absolute_expires_at BIGINT NOT NULL,
    revoked_at BIGINT
);

CREATE INDEX sessions_user ON sessions (user_id, revoked_at);

CREATE TABLE scheduler_lock (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    owner TEXT NOT NULL,
    expires_at BIGINT NOT NULL
);
