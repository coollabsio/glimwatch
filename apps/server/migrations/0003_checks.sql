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
