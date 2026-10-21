-- Agents run checks from their own config inside a private network and report results.

CREATE TABLE agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    token_hash TEXT NOT NULL UNIQUE,
    last_seen_at BIGINT,
    created_at BIGINT NOT NULL
);

ALTER TABLE monitors ADD COLUMN agent_id TEXT REFERENCES agents (id);

-- The protocol an agent declared, such as postgres. job_type stays the transport.
ALTER TABLE monitors ADD COLUMN check_type TEXT;

CREATE UNIQUE INDEX monitors_agent_name ON monitors (agent_id, name);
