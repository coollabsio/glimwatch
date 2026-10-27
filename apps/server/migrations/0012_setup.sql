-- One-time installation setup. Adapted from Orbit's installation_state and setup_tokens.
-- The plaintext token is never stored. Existing databases that already have users are
-- marked initialized so an upgrade does not offer setup again.

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
SELECT 1, 0, NULL, NULL;

UPDATE installation_state
SET initialized = 1,
    initialized_at = (SELECT MIN(created_at) FROM users),
    root_user_id = (
        SELECT id FROM users
        ORDER BY CASE role WHEN 'owner' THEN 0 WHEN 'admin' THEN 1 ELSE 2 END, created_at
        LIMIT 1
    )
WHERE id = 1 AND EXISTS (SELECT 1 FROM users);
