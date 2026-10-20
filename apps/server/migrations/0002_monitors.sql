CREATE TABLE monitors (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    job_type TEXT NOT NULL CHECK (job_type IN ('http', 'tcp')),
    url TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'GET',
    headers_json TEXT NOT NULL DEFAULT '[]',
    body TEXT NOT NULL DEFAULT '',
    timeout_ms BIGINT NOT NULL,
    interval_seconds BIGINT NOT NULL,
    degraded_after_ms BIGINT,
    expected_status BIGINT,
    assertions_json TEXT NOT NULL DEFAULT '[]',
    follow_redirects BIGINT NOT NULL DEFAULT 1,
    failure_threshold BIGINT NOT NULL DEFAULT 3,
    active BIGINT NOT NULL DEFAULT 1,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'up', 'degraded', 'down')),
    consecutive_failures BIGINT NOT NULL DEFAULT 0,
    last_checked_at BIGINT,
    next_check_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX monitors_due ON monitors (active, next_check_at);
