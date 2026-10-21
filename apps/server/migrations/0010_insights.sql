-- Cookieless analytics and short links.
-- Integer columns are BIGINT so one SQL file decodes as i64 on SQLite and Postgres.
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
    created_at BIGINT NOT NULL
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
