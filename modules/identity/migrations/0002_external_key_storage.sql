ALTER TABLE auth_external_applications ADD COLUMN IF NOT EXISTS encrypted_key TEXT NOT NULL DEFAULT '';
CREATE TABLE IF NOT EXISTS auth_external_key_audit (
    id BIGSERIAL PRIMARY KEY,
    client_id TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    action TEXT NOT NULL,
    created_at BIGINT NOT NULL
);
INSERT INTO auth_external_migrations(version, applied_at)
VALUES (2, EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT) ON CONFLICT DO NOTHING;
