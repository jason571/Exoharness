-- Trusted backend identities are namespaced by application, never merged by email.
CREATE TABLE IF NOT EXISTS auth_external_user_links (
    client_id TEXT NOT NULL REFERENCES auth_external_applications(client_id),
    external_subject TEXT NOT NULL,
    user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
    created_at BIGINT NOT NULL,
    PRIMARY KEY(client_id, external_subject),
    UNIQUE(user_id)
);
ALTER TABLE auth_external_grants ADD COLUMN IF NOT EXISTS device_id TEXT;
ALTER TABLE auth_external_grants ADD COLUMN IF NOT EXISTS source TEXT NOT NULL DEFAULT 'browser';
CREATE INDEX IF NOT EXISTS auth_external_backend_grants
    ON auth_external_grants(client_id,user_id,device_id,app_revision,created_at);
INSERT INTO auth_external_migrations(version, applied_at)
VALUES (3, EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT) ON CONFLICT DO NOTHING;
