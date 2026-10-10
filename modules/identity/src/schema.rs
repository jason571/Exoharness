use crate::helpers::hex_bytes;
use crate::types::AuthStore;
use crate::types::{AuthConfiguration, AuthError};
use exoharness_database::{Connection, Database};
use sha2::{Digest, Sha256};
impl AuthStore {
    pub fn open(database: Database) -> Result<Self, AuthError> {
        let store = Self { database };
        let connection = store.connection()?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS auth_login_attempts(bucket TEXT PRIMARY KEY,window_start BIGINT NOT NULL,attempts BIGINT NOT NULL)")?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS auth_passwords (
                 role TEXT PRIMARY KEY NOT NULL,
                 password_hash TEXT NOT NULL,
                 revision BIGINT NOT NULL,
                 updated_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS auth_sessions (
                 token_digest TEXT PRIMARY KEY NOT NULL,
                 role TEXT NOT NULL,
                 auth_revision BIGINT NOT NULL,
                 created_at BIGINT NOT NULL,
                 expires_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS auth_sessions_expiry
                 ON auth_sessions(expires_at);
             CREATE TABLE IF NOT EXISTS auth_users (
                 id TEXT PRIMARY KEY NOT NULL,
                 username TEXT NOT NULL,
                 normalized_username TEXT UNIQUE NOT NULL,
                 email TEXT NOT NULL,
                 normalized_email TEXT UNIQUE NOT NULL,
                 display_name TEXT NOT NULL,
                 password_hash TEXT NOT NULL,
                 default_agent_profile_id TEXT NOT NULL DEFAULT 'xiaolan',
                 revision BIGINT NOT NULL,
                 created_at BIGINT NOT NULL,
                 updated_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS auth_user_sessions (
                 token_digest TEXT PRIMARY KEY NOT NULL,
                 user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
                 auth_revision BIGINT NOT NULL,
                 created_at BIGINT NOT NULL,
                 expires_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS auth_user_sessions_expiry
                 ON auth_user_sessions(expires_at);
             CREATE INDEX IF NOT EXISTS auth_user_sessions_user
                 ON auth_user_sessions(user_id, expires_at);
             CREATE TABLE IF NOT EXISTS auth_user_capability_policy (
                 user_id TEXT PRIMARY KEY NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
                 install_skills BIGINT NOT NULL DEFAULT 0,
                 install_mcp BIGINT NOT NULL DEFAULT 0,
                 install_tools BIGINT NOT NULL DEFAULT 0,
                 use_skills BIGINT NOT NULL DEFAULT 1,
                 use_mcp BIGINT NOT NULL DEFAULT 1,
                 use_tools BIGINT NOT NULL DEFAULT 1,
                 updated_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS auth_user_capability_shares (
                 capability_id TEXT NOT NULL,
                 user_id TEXT NOT NULL REFERENCES auth_users(id) ON DELETE CASCADE,
                 revision BIGINT NOT NULL DEFAULT 0,
                 manifest_digest TEXT NOT NULL DEFAULT '',
                 source_digest TEXT,
                 created_at BIGINT NOT NULL,
                 PRIMARY KEY(capability_id, user_id)
             );
             CREATE INDEX IF NOT EXISTS auth_user_capability_shares_user
                 ON auth_user_capability_shares(user_id);
             CREATE TABLE IF NOT EXISTS auth_email_verifications (
                 id TEXT PRIMARY KEY NOT NULL,
                 email TEXT NOT NULL,
                 normalized_email TEXT NOT NULL,
                 code_hash TEXT NOT NULL,
                 attempt_count BIGINT NOT NULL DEFAULT 0,
                 created_at BIGINT NOT NULL,
                 expires_at BIGINT NOT NULL,
                 activated_at BIGINT,
                 consumed_at BIGINT
             );
             CREATE INDEX IF NOT EXISTS auth_email_verifications_lookup
                 ON auth_email_verifications(normalized_email, activated_at DESC);
             CREATE TABLE IF NOT EXISTS auth_registration_email_settings (
                 singleton_id BIGINT PRIMARY KEY NOT NULL CHECK(singleton_id = 1),
                 subject_template TEXT NOT NULL,
                 body_template TEXT NOT NULL,
                 revision BIGINT NOT NULL,
                 updated_at BIGINT NOT NULL
             );",
        )?;
        // These ALTER statements keep development databases created by an
        // earlier portal build readable. Only newly verified accounts can log
        // in to the public portal.
        connection.execute_batch(
            "ALTER TABLE auth_users ADD COLUMN IF NOT EXISTS email TEXT;
             ALTER TABLE auth_users ADD COLUMN IF NOT EXISTS normalized_email TEXT;
             ALTER TABLE auth_users ADD COLUMN IF NOT EXISTS default_agent_profile_id TEXT;
             ALTER TABLE auth_user_capability_shares ADD COLUMN IF NOT EXISTS revision BIGINT NOT NULL DEFAULT 0;
             ALTER TABLE auth_user_capability_shares ADD COLUMN IF NOT EXISTS manifest_digest TEXT NOT NULL DEFAULT '';
             ALTER TABLE auth_user_capability_shares ADD COLUMN IF NOT EXISTS source_digest TEXT;
             UPDATE auth_users SET default_agent_profile_id='xiaolan'
                 WHERE default_agent_profile_id IS NULL OR btrim(default_agent_profile_id)='';
             CREATE UNIQUE INDEX IF NOT EXISTS auth_users_normalized_email
                 ON auth_users(normalized_email) WHERE normalized_email IS NOT NULL;",
        )?;
        Ok(store)
    }

    pub fn configuration(&self) -> Result<AuthConfiguration, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT role FROM auth_passwords")?;
        let roles = statement
            .query_map((), |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(AuthConfiguration {
            admin_configured: roles.iter().any(|role| role == "admin"),
            chat_configured: roles.iter().any(|role| role == "chat"),
        })
    }

    pub(crate) fn connection(&self) -> Result<Connection, AuthError> {
        self.database.connection().map_err(Into::into)
    }
}

pub fn csrf_token(session_token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"exoharness-csrf-v1\0");
    digest.update(session_token.as_bytes());
    hex_bytes(&digest.finalize())
}
