-- Owlee schema for SQLite and Postgres.
-- Integer columns are BIGINT so the same SQL decodes as i64 on both engines.

-- Accounts, sessions, and the single-runner scheduler lease.

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

-- Agents run checks from their own config inside a private network and report
-- results. agent_id ties a monitor to an agent. check_type is the protocol the
-- agent declared, such as postgres. job_type stays the transport.

CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL UNIQUE,
    last_seen_at BIGINT,
    created_at BIGINT NOT NULL
);

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
    updated_at BIGINT NOT NULL,
    agent_id TEXT REFERENCES agents (id),
    check_type TEXT
);

CREATE INDEX monitors_due ON monitors (active, next_check_at);

CREATE UNIQUE INDEX monitors_agent_name ON monitors (agent_id, name);

CREATE TABLE check_results (
    id TEXT PRIMARY KEY,
    monitor_id TEXT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('success', 'degraded', 'failure', 'maintenance')),
    status_code BIGINT,
    latency_ms BIGINT NOT NULL,
    error TEXT,
    region TEXT NOT NULL DEFAULT 'local',
    checked_at BIGINT NOT NULL
);

CREATE INDEX check_results_monitor_time ON check_results (monitor_id, checked_at);

CREATE TABLE monitor_daily_stats (
    monitor_id TEXT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    success BIGINT NOT NULL DEFAULT 0,
    degraded BIGINT NOT NULL DEFAULT 0,
    failure BIGINT NOT NULL DEFAULT 0,
    maintenance BIGINT NOT NULL DEFAULT 0,
    latency_sum BIGINT NOT NULL DEFAULT 0,
    latency_count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (monitor_id, day)
);

CREATE TABLE status_pages (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    slug TEXT NOT NULL UNIQUE,
    published BIGINT NOT NULL DEFAULT 0,
    homepage_url TEXT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE page_components (
    id TEXT PRIMARY KEY,
    page_id TEXT NOT NULL REFERENCES status_pages (id) ON DELETE CASCADE,
    monitor_id TEXT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    position BIGINT NOT NULL DEFAULT 0,
    created_at BIGINT NOT NULL,
    UNIQUE (page_id, monitor_id)
);

CREATE INDEX page_components_order ON page_components (page_id, position);

CREATE TABLE incidents (
    id TEXT PRIMARY KEY,
    page_id TEXT REFERENCES status_pages (id) ON DELETE SET NULL,
    monitor_id TEXT REFERENCES monitors (id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('investigating', 'identified', 'monitoring', 'resolved')),
    impact TEXT NOT NULL CHECK (impact IN ('none', 'minor', 'major', 'critical')),
    auto BIGINT NOT NULL DEFAULT 0,
    started_at BIGINT NOT NULL,
    resolved_at BIGINT,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX incidents_status ON incidents (status, started_at);
CREATE INDEX incidents_monitor ON incidents (monitor_id, status);

CREATE TABLE incident_updates (
    id TEXT PRIMARY KEY,
    incident_id TEXT NOT NULL REFERENCES incidents (id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('investigating', 'identified', 'monitoring', 'resolved')),
    message TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE INDEX incident_updates_incident ON incident_updates (incident_id, created_at);

CREATE TABLE maintenances (
    id TEXT PRIMARY KEY,
    page_id TEXT REFERENCES status_pages (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    message TEXT NOT NULL,
    starts_at BIGINT NOT NULL,
    ends_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX maintenances_window ON maintenances (starts_at, ends_at);

CREATE TABLE maintenance_monitors (
    maintenance_id TEXT NOT NULL REFERENCES maintenances (id) ON DELETE CASCADE,
    monitor_id TEXT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    PRIMARY KEY (maintenance_id, monitor_id)
);

CREATE TABLE notifications (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider IN ('email', 'webhook', 'slack', 'discord')),
    config_json TEXT NOT NULL,
    enabled BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE TABLE notification_monitors (
    notification_id TEXT NOT NULL REFERENCES notifications (id) ON DELETE CASCADE,
    monitor_id TEXT NOT NULL REFERENCES monitors (id) ON DELETE CASCADE,
    PRIMARY KEY (notification_id, monitor_id)
);

CREATE TABLE notification_deliveries (
    id TEXT PRIMARY KEY,
    notification_id TEXT NOT NULL REFERENCES notifications (id) ON DELETE CASCADE,
    monitor_id TEXT,
    incident_id TEXT,
    event TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('sent', 'failed', 'skipped')),
    error TEXT,
    created_at BIGINT NOT NULL
);

CREATE INDEX notification_deliveries_time ON notification_deliveries (notification_id, created_at);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY,
    actor_id TEXT,
    action TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    occurred_at BIGINT NOT NULL
);

CREATE INDEX audit_events_time ON audit_events (occurred_at);

-- Cookieless analytics and short links.
-- Visitor addresses are not stored. session_id is a monthly hash.

CREATE TABLE insight_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE insight_sites (
    id TEXT PRIMARY KEY,
    public_id TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    domain TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#4f46e5',
    public_slug TEXT NOT NULL UNIQUE,
    public_enabled BIGINT NOT NULL DEFAULT 0,
    public_threshold BIGINT NOT NULL DEFAULT 5,
    public_sections TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    verified BIGINT NOT NULL DEFAULT 0,
    verify_token TEXT NOT NULL DEFAULT '',
    verified_at BIGINT,
    -- NULL inherits the instance analytics cap.
    daily_quota BIGINT
);

CREATE TABLE insight_domains (
    id TEXT PRIMARY KEY,
    site_id TEXT REFERENCES insight_sites (id) ON DELETE SET NULL,
    hostname TEXT NOT NULL UNIQUE,
    note TEXT NOT NULL DEFAULT '',
    enabled BIGINT NOT NULL DEFAULT 1,
    created_at BIGINT NOT NULL
);

CREATE TABLE insight_links (
    id TEXT PRIMARY KEY,
    domain_id TEXT NOT NULL REFERENCES insight_domains (id) ON DELETE CASCADE,
    slug TEXT NOT NULL,
    destination TEXT NOT NULL,
    redirect_type TEXT NOT NULL DEFAULT 'temporary',
    enabled BIGINT NOT NULL DEFAULT 1,
    public_slug TEXT NOT NULL UNIQUE,
    public_enabled BIGINT NOT NULL DEFAULT 0,
    public_threshold BIGINT NOT NULL DEFAULT 5,
    public_sections TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    UNIQUE (domain_id, slug)
);

CREATE INDEX insight_links_domain_slug ON insight_links (domain_id, slug);

CREATE TABLE insight_events (
    id TEXT PRIMARY KEY,
    site_id TEXT REFERENCES insight_sites (id) ON DELETE CASCADE,
    link_id TEXT REFERENCES insight_links (id) ON DELETE CASCADE,
    timestamp BIGINT NOT NULL,
    event_type TEXT NOT NULL,
    session_id TEXT,
    visit_id TEXT,
    pathname TEXT,
    url_query TEXT,
    title TEXT,
    hostname TEXT,
    utm_source TEXT,
    utm_medium TEXT,
    utm_campaign TEXT,
    utm_content TEXT,
    utm_term TEXT,
    referrer_domain TEXT,
    country TEXT,
    region TEXT,
    city TEXT,
    browser TEXT,
    os TEXT,
    device TEXT,
    screen TEXT,
    language TEXT,
    metadata TEXT,
    lcp DOUBLE PRECISION,
    inp DOUBLE PRECISION,
    cls DOUBLE PRECISION,
    fcp DOUBLE PRECISION,
    ttfb DOUBLE PRECISION,
    CHECK (site_id IS NOT NULL OR link_id IS NOT NULL)
);

-- Raw pageviews and link clicks share this table. Retention deletes by timestamp.
CREATE INDEX insight_events_timestamp ON insight_events (timestamp);

CREATE INDEX insight_events_site_time ON insight_events (site_id, timestamp);
CREATE INDEX insight_events_link_time ON insight_events (link_id, timestamp);
CREATE INDEX insight_events_site_path ON insight_events (site_id, pathname, timestamp);

CREATE TABLE insight_visits (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES insight_sites (id) ON DELETE CASCADE,
    session_id TEXT NOT NULL,
    started_at BIGINT NOT NULL,
    last_at BIGINT NOT NULL,
    pageviews BIGINT NOT NULL DEFAULT 0,
    events BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX insight_visits_session ON insight_visits (site_id, session_id, last_at);

CREATE TABLE insight_daily (
    site_id TEXT NOT NULL REFERENCES insight_sites (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    pageviews BIGINT NOT NULL DEFAULT 0,
    visitors BIGINT NOT NULL DEFAULT 0,
    events BIGINT NOT NULL DEFAULT 0,
    visits BIGINT NOT NULL DEFAULT 0,
    bounces BIGINT NOT NULL DEFAULT 0,
    total_time BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (site_id, day)
);

CREATE TABLE insight_link_daily (
    link_id TEXT NOT NULL REFERENCES insight_links (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    clicks BIGINT NOT NULL DEFAULT 0,
    visitors BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (link_id, day)
);

CREATE TABLE insight_dimension_daily (
    site_id TEXT NOT NULL REFERENCES insight_sites (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    dimension TEXT NOT NULL,
    value TEXT NOT NULL,
    count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (site_id, day, dimension, value)
);

CREATE INDEX insight_dimension_query ON insight_dimension_daily (site_id, dimension, day);

CREATE TABLE insight_link_dimension_daily (
    link_id TEXT NOT NULL REFERENCES insight_links (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    dimension TEXT NOT NULL,
    value TEXT NOT NULL,
    count BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (link_id, day, dimension, value)
);

CREATE TABLE insight_site_sessions (
    site_id TEXT NOT NULL REFERENCES insight_sites (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    session_id TEXT NOT NULL,
    PRIMARY KEY (site_id, day, session_id)
);

CREATE TABLE insight_link_sessions (
    link_id TEXT NOT NULL REFERENCES insight_links (id) ON DELETE CASCADE,
    day BIGINT NOT NULL,
    session_id TEXT NOT NULL,
    PRIMARY KEY (link_id, day, session_id)
);

CREATE TABLE insight_event_data (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL REFERENCES insight_sites (id) ON DELETE CASCADE,
    event_id TEXT NOT NULL REFERENCES insight_events (id) ON DELETE CASCADE,
    event_name TEXT NOT NULL,
    timestamp BIGINT NOT NULL,
    key TEXT NOT NULL,
    string_value TEXT NOT NULL,
    number_value DOUBLE PRECISION,
    data_type BIGINT NOT NULL
);

CREATE INDEX insight_event_data_name ON insight_event_data (site_id, event_name, timestamp);

-- Error tracking. Timestamps stay RFC3339 text, matching the Sentry event clock.
-- Dashboard accounts stay in users. These tables are projects, issues, and events.

CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    public_key TEXT NOT NULL UNIQUE,
    platform TEXT NOT NULL,
    created_at TEXT NOT NULL,
    next_issue BIGINT NOT NULL DEFAULT 1,
    -- NULL inherits the instance error cap.
    daily_quota BIGINT
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
    stored_count BIGINT NOT NULL DEFAULT 0,
    sample_window_start TEXT,
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

-- Retention deletes and the 24h trend (`WHERE timestamp >= ?`) both scan by time.
CREATE INDEX events_timestamp ON events (timestamp);

CREATE INDEX events_project ON events (project_id);

-- One-time installation setup. The plaintext token is never stored.
-- One row, id = 1. initialized stays 0 until the owner account is created.

CREATE TABLE installation_state (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    initialized BIGINT NOT NULL CHECK (initialized IN (0, 1)),
    initialized_at BIGINT,
    root_user_id TEXT REFERENCES users (id)
);

CREATE TABLE setup_tokens (
    token_hash TEXT PRIMARY KEY,
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

INSERT INTO installation_state (id, initialized, initialized_at, root_user_id)
VALUES (1, 0, NULL, NULL);

-- Instance settings the root user changes in Settings: open registration and the
-- mail server required before registration can open. One row, id = 1.
-- Registration starts closed.

CREATE TABLE instance_settings (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    registration_open BIGINT NOT NULL CHECK (registration_open IN (0, 1)),
    smtp_host TEXT,
    smtp_port BIGINT,
    smtp_username TEXT,
    smtp_password TEXT,
    smtp_from TEXT,
    updated_at BIGINT,
    -- Safety valve, not a bill. One error a second all day is about 86,400.
    error_daily_quota BIGINT NOT NULL DEFAULT 100000,
    -- A million pageviews a day is a large site for a self-hosted install.
    analytics_daily_quota BIGINT NOT NULL DEFAULT 1000000,
    quotas_updated_at BIGINT
);

CREATE TABLE ingest_usage (
    kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    day TEXT NOT NULL,
    count BIGINT NOT NULL,
    PRIMARY KEY (kind, subject_id, day)
);

INSERT INTO instance_settings (id, registration_open) VALUES (1, 0);
