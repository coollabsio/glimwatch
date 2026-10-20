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
