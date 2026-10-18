CREATE TABLE IF NOT EXISTS execution_external_sessions (
    session_id TEXT PRIMARY KEY REFERENCES execution_sessions(id),
    client_id TEXT NOT NULL,
    owner_user_id TEXT NOT NULL,
    assistant_id TEXT NOT NULL,
    request_key_digest TEXT NOT NULL,
    request_digest TEXT NOT NULL,
    UNIQUE(client_id, owner_user_id, request_key_digest)
);
CREATE INDEX IF NOT EXISTS execution_external_sessions_owner
    ON execution_external_sessions(owner_user_id, client_id);
