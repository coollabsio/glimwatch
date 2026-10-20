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
