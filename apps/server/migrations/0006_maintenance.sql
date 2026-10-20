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
