-- Optional caller-supplied metadata; account ownership remains authoritative.
ALTER TABLE execution_external_sessions ADD COLUMN IF NOT EXISTS device_id TEXT;
CREATE INDEX IF NOT EXISTS execution_external_sessions_device
    ON execution_external_sessions(client_id, device_id, owner_user_id);
CREATE INDEX IF NOT EXISTS execution_external_sessions_application
    ON execution_external_sessions(client_id, session_id);
