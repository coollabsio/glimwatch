-- Error tracking. The upstream SQLite and Postgres migrations were the same SQL,
-- so this file is applied on both engines. Timestamps stay RFC3339 text, matching
-- the Sentry event clock. Dashboard accounts stay in `users`; these tables are
-- only projects, issues, and events.

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL UNIQUE,
    platform TEXT NOT NULL,
    created_at TEXT NOT NULL,
    next_issue BIGINT NOT NULL DEFAULT 1
);

CREATE TABLE project_counters (
    id BIGINT PRIMARY KEY,
    next_id BIGINT NOT NULL
);

INSERT INTO project_counters (id, next_id) VALUES (1, 1);

CREATE TABLE issues (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    title TEXT NOT NULL,
    culprit TEXT,
    status TEXT NOT NULL,
    level TEXT NOT NULL,
    platform TEXT,
    environment TEXT,
    release TEXT,
    first_seen TEXT NOT NULL,
    last_seen TEXT NOT NULL,
    event_count BIGINT NOT NULL DEFAULT 0,
    short_id TEXT,
    unhandled BIGINT NOT NULL DEFAULT 0,
    UNIQUE (project_id, fingerprint)
);

CREATE UNIQUE INDEX issues_short_id ON issues (short_id);

CREATE INDEX issues_last_seen ON issues (last_seen);

CREATE INDEX issues_project ON issues (project_id);

CREATE TABLE events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    issue_id TEXT NOT NULL REFERENCES issues (id) ON DELETE CASCADE,
    timestamp TEXT NOT NULL,
    level TEXT NOT NULL,
    title TEXT NOT NULL,
    environment TEXT,
    release TEXT,
    user_key TEXT,
    payload TEXT NOT NULL
);

CREATE INDEX events_issue_timestamp ON events (issue_id, timestamp);

CREATE INDEX events_project ON events (project_id);
