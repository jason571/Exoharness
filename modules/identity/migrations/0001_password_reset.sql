CREATE TABLE auth_password_resets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
    token_digest TEXT NOT NULL UNIQUE,
    auth_revision BIGINT NOT NULL,
    created_at BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    activated_at BIGINT,
    consumed_at BIGINT
);
CREATE INDEX auth_password_resets_user ON auth_password_resets(user_id, created_at DESC);
