-- Retain account mappings and conversation ownership after deleting an app.
ALTER TABLE auth_external_applications ADD COLUMN IF NOT EXISTS deleted_at BIGINT;
INSERT INTO auth_external_migrations(version, applied_at)
VALUES (4, EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT) ON CONFLICT DO NOTHING;
