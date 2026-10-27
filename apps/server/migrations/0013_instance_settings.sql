-- Instance settings the root user changes in Settings: open registration and the
-- mail server required before registration can open. One row, id = 1.
-- Registration starts closed. STATIXO_OPEN_SIGNUP, HARUVO_OPEN_SIGNUP, and
-- STATUS_OPEN_SIGNUP do not open it.

CREATE TABLE instance_settings (
    id BIGINT PRIMARY KEY CHECK (id = 1),
    registration_open BIGINT NOT NULL CHECK (registration_open IN (0, 1)),
    smtp_host TEXT,
    smtp_port BIGINT,
    smtp_username TEXT,
    smtp_password TEXT,
    smtp_from TEXT,
    updated_at BIGINT
);

INSERT INTO instance_settings (id, registration_open) VALUES (1, 0);
