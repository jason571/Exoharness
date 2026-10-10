// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::helpers::{
    hash_password, now_seconds, random_session_token, token_digest, validate_display_name,
    validate_password, verify_password,
};
use crate::types::AuthStore;
use crate::types::{
    AuthError, DEFAULT_AGENT_PROFILE_ID, MAX_EMAIL_BYTES, MAX_PASSWORD_BYTES, UserAccount,
    UserAuthSession, ValidatedUserSession,
};
use exoharness_database::OptionalExtension;
use exoharness_database::params;
impl AuthStore {
    pub fn assistant_reference_count(&self, id: &str) -> Result<i64, AuthError> {
        Ok(self.connection()?.query_row("SELECT COUNT(*) FROM auth_users WHERE COALESCE(NULLIF(btrim(default_agent_profile_id), ''), ?1)=?2",params![DEFAULT_AGENT_PROFILE_ID,id],|row|row.get(0))?)
    }
    pub fn authenticate_user(
        &self,
        identifier: &str,
        password: &str,
        ttl_seconds: i64,
    ) -> Result<Option<UserAuthSession>, AuthError> {
        if password.is_empty() || password.len() > MAX_PASSWORD_BYTES {
            return Ok(None);
        }
        let identifier = identifier.trim().to_ascii_lowercase();
        if identifier.is_empty() || identifier.len() > MAX_EMAIL_BYTES {
            return Ok(None);
        }
        let connection = self.connection()?;
        let transaction = connection.transaction()?;
        let record = transaction
            .query_row(
                "SELECT id, username, COALESCE(email, ''), display_name, password_hash,
                        revision, created_at, updated_at
                 FROM auth_users WHERE normalized_username=?1 OR normalized_email=?1 FOR UPDATE",
                params![identifier],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            id,
            username,
            email,
            display_name,
            password_hash,
            revision,
            created_at,
            updated_at,
        )) = record
        else {
            return Ok(None);
        };
        if !verify_password(password, &password_hash) {
            return Ok(None);
        }
        let (token, token_digest) = random_session_token()?;
        let now = now_seconds();
        let expires_at = now.saturating_add(ttl_seconds);
        transaction.execute(
            "DELETE FROM auth_user_sessions WHERE expires_at <= ?1",
            [now],
        )?;
        transaction.execute(
            "INSERT INTO auth_user_sessions(
                 token_digest, user_id, auth_revision, created_at, expires_at
             ) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![token_digest, id, revision, now, expires_at],
        )?;
        transaction.commit()?;
        Ok(Some(UserAuthSession {
            token,
            account: UserAccount {
                id,
                username,
                email,
                display_name,
                created_at,
                updated_at,
            },
            expires_at,
        }))
    }

    pub fn validate_user_session(
        &self,
        token: &str,
    ) -> Result<Option<ValidatedUserSession>, AuthError> {
        if token.len() > 256 {
            return Ok(None);
        }
        let digest = token_digest(token);
        let now = now_seconds();
        let record = self
            .connection()?
            .query_row(
                "SELECT u.id, u.username, COALESCE(u.email, ''), u.display_name,
                        u.created_at, u.updated_at, s.expires_at
                 FROM auth_user_sessions s
                 JOIN auth_users u ON u.id=s.user_id AND u.revision=s.auth_revision
                 WHERE s.token_digest=?1 AND s.expires_at>?2",
                params![digest, now],
                |row| {
                    Ok((
                        UserAccount {
                            id: row.get(0)?,
                            username: row.get(1)?,
                            email: row.get(2)?,
                            display_name: row.get(3)?,
                            created_at: row.get(4)?,
                            updated_at: row.get(5)?,
                        },
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        Ok(record.map(|(account, expires_at)| ValidatedUserSession {
            account,
            expires_at,
            token_digest: digest,
        }))
    }

    pub fn update_user_profile(
        &self,
        user_id: &str,
        display_name: &str,
    ) -> Result<UserAccount, AuthError> {
        let display_name = validate_display_name(display_name)?;
        let now = now_seconds();
        let connection = self.connection()?;
        let updated = connection.execute(
            "UPDATE auth_users SET display_name=?2, updated_at=?3 WHERE id=?1",
            params![user_id, display_name, now],
        )?;
        if updated != 1 {
            return Err(AuthError::Validation("用户不存在".into()));
        }
        self.user_account(user_id)?
            .ok_or_else(|| AuthError::Validation("用户不存在".into()))
    }

    pub fn update_user_password(
        &self,
        user_id: &str,
        current_password: &str,
        new_password: &str,
    ) -> Result<bool, AuthError> {
        validate_password(new_password)?;
        let connection = self.connection()?;
        let current_hash = connection
            .query_row(
                "SELECT password_hash FROM auth_users WHERE id=?1",
                params![user_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(current_hash) = current_hash else {
            return Ok(false);
        };
        if !verify_password(current_password, &current_hash) {
            return Ok(false);
        }
        let password_hash = hash_password(new_password)?;
        let transaction = connection.transaction()?;
        let changed = transaction.execute(
            "UPDATE auth_users SET password_hash=?2, revision=revision+1, updated_at=?3
             WHERE id=?1 AND password_hash=?4",
            params![user_id, password_hash, now_seconds(), current_hash],
        )?;
        if changed != 1 {
            return Ok(false);
        }
        transaction.execute(
            "DELETE FROM auth_user_sessions WHERE user_id=?1",
            params![user_id],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn delete_user_session_digest(&self, digest: &str) -> Result<(), AuthError> {
        self.connection()?.execute(
            "DELETE FROM auth_user_sessions WHERE token_digest=?1",
            params![digest],
        )?;
        Ok(())
    }

    pub fn matching_user_ids(&self, name: &str) -> Result<Vec<String>, AuthError> {
        let connection = self.connection()?;
        // Substring matching is literal: '%' and '_' are not wildcard operators.
        let mut query = connection.prepare(
            "SELECT id FROM auth_users WHERE strpos(lower(display_name || ' ' || username), lower(?1::TEXT)) > 0 ORDER BY id",
        )?;
        let mut ids = query
            .query_map(params![name], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        if "管理端".contains(name) || "management".contains(&name.to_lowercase()) {
            ids.push("management".into());
        }
        Ok(ids)
    }

    /// Returns the administrator-facing account projection and its default
    /// assistant assignment. Authentication secrets are intentionally absent.
    pub fn management_users(&self) -> Result<Vec<(UserAccount, String)>, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, username, COALESCE(email, ''), display_name,
                    created_at, updated_at,
                    COALESCE(NULLIF(btrim(default_agent_profile_id), ''), ?1)
             FROM auth_users ORDER BY created_at DESC, id DESC",
        )?;
        statement
            .query_map(params![DEFAULT_AGENT_PROFILE_ID], |row| {
                Ok((
                    UserAccount {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        email: row.get(2)?,
                        display_name: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    },
                    row.get(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn user_labels(
        &self,
        ids: &[String],
    ) -> Result<std::collections::BTreeMap<String, String>, AuthError> {
        let encoded =
            serde_json::to_string(ids).map_err(|error| AuthError::Validation(error.to_string()))?;
        let connection = self.connection()?;
        // Bind as text before casting: the database driver receives a Rust String,
        // not a native PostgreSQL JSONB parameter.
        let mut query = connection.prepare(
            "SELECT id,display_name || ' · ' || username FROM auth_users
            WHERE id IN (SELECT jsonb_array_elements_text(?1::TEXT::jsonb))",
        )?;
        query
            .query_map(params![encoded], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()
            .map_err(Into::into)
    }

    pub fn user_account(&self, user_id: &str) -> Result<Option<UserAccount>, AuthError> {
        self.connection()?
            .query_row(
                "SELECT id, username, COALESCE(email, ''), display_name, created_at, updated_at
                 FROM auth_users WHERE id=?1",
                params![user_id],
                |row| {
                    Ok(UserAccount {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        email: row.get(2)?,
                        display_name: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }
}
