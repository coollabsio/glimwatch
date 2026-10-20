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
