-- Additive migration: registered accounts and browser sessions are retained.
CREATE TABLE IF NOT EXISTS auth_external_applications (
    client_id TEXT PRIMARY KEY,
    configuration_json TEXT NOT NULL,
    secret_digest TEXT NOT NULL,
    revision BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS auth_external_grants (
    id TEXT PRIMARY KEY,
    client_id TEXT NOT NULL REFERENCES auth_external_applications(client_id),
    user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
    app_revision BIGINT NOT NULL,
    auth_revision BIGINT NOT NULL,
    scopes_json TEXT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    revoked_at BIGINT
);
CREATE INDEX IF NOT EXISTS auth_external_grants_user ON auth_external_grants(user_id, client_id);
CREATE TABLE IF NOT EXISTS auth_external_codes (
    code_digest TEXT PRIMARY KEY,
    grant_id TEXT NOT NULL REFERENCES auth_external_grants(id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    code_challenge TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    consumed_at BIGINT
);
CREATE TABLE IF NOT EXISTS auth_external_tokens (
    token_digest TEXT PRIMARY KEY,
    grant_id TEXT NOT NULL REFERENCES auth_external_grants(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK(kind IN ('access', 'refresh')),
    expires_at BIGINT NOT NULL,
    consumed_at BIGINT
);
CREATE INDEX IF NOT EXISTS auth_external_tokens_grant ON auth_external_tokens(grant_id);
CREATE TABLE IF NOT EXISTS auth_external_migrations (
    version BIGINT PRIMARY KEY,
    applied_at BIGINT NOT NULL
);
INSERT INTO auth_external_migrations(version, applied_at)
VALUES (1, EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::BIGINT) ON CONFLICT DO NOTHING;
