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
