use crate::helpers::{
    hash_password, now_seconds, opposite_role, token_digest, validate_password, verify_password,
};
use crate::types::AuthStore;
use crate::types::{
    AuthError, AuthRole, AuthSession, MAX_PASSWORD_BYTES, SESSION_TOKEN_BYTES, ValidatedSession,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use exoharness_database::OptionalExtension;
use exoharness_database::params;
impl AuthStore {
    /// Seed exactly once. Restarting with a stale credential file cannot undo rotation.
    pub fn bootstrap_password(&self, role: AuthRole, password: &str) -> Result<(), AuthError> {
        validate_password(password)?;
        let encoded = hash_password(password)?;
        let connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "SELECT pg_advisory_xact_lock(hashtext('auth_password_roles'))",
            (),
        )?;
        let exists = transaction.query_row(
            "SELECT COUNT(*) FROM auth_passwords WHERE role=?1",
            [role.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        if exists == 0 {
            if let Some(other) = transaction
                .query_row(
                    "SELECT password_hash FROM auth_passwords WHERE role=?1",
                    [opposite_role(role).as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                && verify_password(password, &other)
            {
                return Err(AuthError::Validation(
                    "管理员密码与只读用户密码必须不同".into(),
                ));
            }
            transaction.execute("INSERT INTO auth_passwords(role,password_hash,revision,updated_at) VALUES(?1,?2,1,?3)", params![role.as_str(),encoded,now_seconds()])?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Verification and session insertion share the role-row lock.
    pub fn authenticate_session(
        &self,
        password: &str,
        ttl_seconds: i64,
    ) -> Result<Option<AuthSession>, AuthError> {
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return Ok(None);
        }
        if !(1..=31 * 24 * 3600).contains(&ttl_seconds) {
            return Err(AuthError::Validation("会话有效期无效".into()));
        }
        let connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "SELECT pg_advisory_xact_lock(hashtext('auth_password_roles'))",
            (),
        )?;
        for role in [AuthRole::Admin, AuthRole::Chat] {
            let record = transaction
                .query_row(
                    "SELECT password_hash,revision FROM auth_passwords WHERE role=?1 FOR UPDATE",
                    [role.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
                )
                .optional()?;
            if let Some((hash, revision)) = record
                && verify_password(password, &hash)
            {
                let (token, digest) = crate::helpers::random_session_token()?;
                let now = now_seconds();
                let expires_at = now.saturating_add(ttl_seconds);
                transaction.execute("DELETE FROM auth_sessions WHERE expires_at<=?1", [now])?;
                transaction.execute("INSERT INTO auth_sessions(token_digest,role,auth_revision,created_at,expires_at) VALUES(?1,?2,?3,?4,?5)",params![digest,role.as_str(),revision,now,expires_at])?;
                transaction.commit()?;
                return Ok(Some(AuthSession {
                    token,
                    role,
                    expires_at,
                }));
            }
        }
        Ok(None)
    }

    pub fn rotate_role_password(
        &self,
        current_admin_password: &str,
        role: AuthRole,
        password: &str,
    ) -> Result<bool, AuthError> {
        validate_password(password)?;
        if current_admin_password.len() > MAX_PASSWORD_BYTES {
            return Ok(false);
        }
        let encoded = hash_password(password)?;
        let connection = self.connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "SELECT pg_advisory_xact_lock(hashtext('auth_password_roles'))",
            (),
        )?;
        let admin = transaction
            .query_row(
                "SELECT password_hash FROM auth_passwords WHERE role='admin' FOR UPDATE",
                (),
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if !admin.is_some_and(|hash| verify_password(current_admin_password, &hash)) {
            return Ok(false);
        }
        if let Some(other) = transaction
            .query_row(
                "SELECT password_hash FROM auth_passwords WHERE role=?1",
                [opposite_role(role).as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            && verify_password(password, &other)
        {
            return Err(AuthError::Validation(
                "管理员密码与只读用户密码必须不同".into(),
            ));
        }
        transaction.execute("INSERT INTO auth_passwords(role,password_hash,revision,updated_at) VALUES(?1,?2,1,?3) ON CONFLICT(role) DO UPDATE SET password_hash=excluded.password_hash,revision=auth_passwords.revision+1,updated_at=excluded.updated_at",params![role.as_str(),encoded,now_seconds()])?;
        transaction.execute("DELETE FROM auth_sessions WHERE role=?1", [role.as_str()])?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn set_password(&self, role: AuthRole, password: &str) -> Result<(), AuthError> {
        validate_password(password)?;
        let password_hash = hash_password(password)?;
        let connection = self.connection()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "SELECT pg_advisory_xact_lock(hashtext('auth_password_roles'))",
            (),
        )?;
        if let Some(other_hash) = transaction
            .query_row(
                "SELECT password_hash FROM auth_passwords WHERE role = ?1",
                [opposite_role(role).as_str()],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            && verify_password(password, &other_hash)
        {
            return Err(AuthError::Validation(
                "管理员密码与只读用户密码必须不同".into(),
            ));
        }
        transaction.execute(
            "INSERT INTO auth_passwords(role, password_hash, revision, updated_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(role) DO UPDATE SET
                 password_hash = excluded.password_hash,
                 revision = auth_passwords.revision + 1,
                 updated_at = excluded.updated_at",
            params![role.as_str(), password_hash, now_seconds()],
        )?;
        transaction.execute("DELETE FROM auth_sessions WHERE role = ?1", [role.as_str()])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn authenticate(&self, password: &str) -> Result<Option<AuthRole>, AuthError> {
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return Ok(None);
        }
        let connection = self.connection()?;
        for role in [AuthRole::Admin, AuthRole::Chat] {
            let password_hash = connection
                .query_row(
                    "SELECT password_hash FROM auth_passwords WHERE role = ?1",
                    [role.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            if password_hash
                .as_deref()
                .is_some_and(|hash| verify_password(password, hash))
            {
                return Ok(Some(role));
            }
        }
        Ok(None)
    }

    pub fn create_session(
        &self,
        role: AuthRole,
        ttl_seconds: i64,
    ) -> Result<AuthSession, AuthError> {
        let mut token_bytes = [0_u8; SESSION_TOKEN_BYTES];
        getrandom::fill(&mut token_bytes).map_err(|_| AuthError::Random)?;
        let token = URL_SAFE_NO_PAD.encode(token_bytes);
        let token_digest = token_digest(&token);
        let now = now_seconds();
        let expires_at = now.saturating_add(ttl_seconds);
        let connection = self.connection()?;
        let revision = connection.query_row(
            "SELECT revision FROM auth_passwords WHERE role = ?1",
            [role.as_str()],
            |row| row.get::<_, i64>(0),
        )?;
        connection.execute("DELETE FROM auth_sessions WHERE expires_at <= ?1", [now])?;
        connection.execute(
            "INSERT INTO auth_sessions(
                 token_digest, role, auth_revision, created_at, expires_at
             ) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![token_digest, role.as_str(), revision, now, expires_at],
        )?;
        Ok(AuthSession {
            token,
            role,
            expires_at,
        })
    }

    pub fn validate_session(&self, token: &str) -> Result<Option<ValidatedSession>, AuthError> {
        if token.len() > 256 {
            return Ok(None);
        }
        let digest = token_digest(token);
        let now = now_seconds();
        let connection = self.connection()?;
        let record = connection
            .query_row(
                "SELECT s.role, s.expires_at
                 FROM auth_sessions s
                 JOIN auth_passwords p ON p.role = s.role
                    AND p.revision = s.auth_revision
                 WHERE s.token_digest = ?1 AND s.expires_at > ?2",
                params![digest, now],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        Ok(record.and_then(|(role, expires_at)| {
            AuthRole::parse(&role).map(|role| ValidatedSession {
                role,
                expires_at,
                token_digest: digest,
            })
        }))
    }

    pub fn delete_session_digest(&self, digest: &str) -> Result<(), AuthError> {
        self.connection()?.execute(
            "DELETE FROM auth_sessions WHERE token_digest = ?1",
            [digest],
        )?;
        Ok(())
    }
}
