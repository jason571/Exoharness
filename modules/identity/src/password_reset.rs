// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    AuthError, AuthStore,
    helpers::{
        hash_password, now_seconds, random_session_token, random_verification_id, token_digest,
        validate_email, validate_password,
    },
};
use exoharness_database::{OptionalExtension, params};
#[cfg(test)]
#[path = "../tests/unit/password_reset.rs"]
mod tests;

pub const PASSWORD_RESET_TTL_SECONDS: i64 = 20 * 60;
pub const PASSWORD_RESET_RESEND_SECONDS: i64 = 60;

pub struct IssuedPasswordReset {
    pub id: String,
    pub email: String,
    pub token: String,
}
fn invalid() -> AuthError {
    AuthError::PasswordResetInvalid
}
fn digest(token: &str) -> Result<String, AuthError> {
    if !(32..=256).contains(&token.len())
        || !token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(invalid());
    }
    Ok(token_digest(&format!("password-reset:{token}")))
}

pub(crate) fn migrate(store: &AuthStore) -> Result<(), AuthError> {
    let tx = store.connection()?.transaction()?;
    tx.query_row("SELECT pg_advisory_xact_lock(hashtextextended(current_schema() || ':identity-password-reset',4271072006)),1::BIGINT",(),|r|r.get::<_,i64>(1))?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS auth_identity_migrations(version BIGINT PRIMARY KEY, applied_at BIGINT NOT NULL)")?;
    let applied = tx
        .query_row(
            "SELECT version FROM auth_identity_migrations WHERE version=1",
            (),
            |r| r.get::<_, i64>(0),
        )
        .optional()?
        .is_some();
    if !applied {
        tx.execute_batch(include_str!("../migrations/0001_password_reset.sql"))?;
        tx.execute(
            "INSERT INTO auth_identity_migrations(version,applied_at) VALUES(1,?1)",
            [now_seconds()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

impl AuthStore {
    pub fn issue_password_reset(
        &self,
        email: &str,
    ) -> Result<Option<IssuedPasswordReset>, AuthError> {
        let (_, normalized) = validate_email(email)?;
        let tx = self.connection()?.transaction()?;
        let user = tx
            .query_row(
                "SELECT id,email FROM auth_users WHERE normalized_email=?1",
                [normalized],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((user, email)) = user else {
            return Ok(None);
        };
        tx.query_row(
            "SELECT pg_advisory_xact_lock(hashtextextended(?1::TEXT,4271072007)),1::BIGINT",
            [user.clone()],
            |r| r.get::<_, i64>(1),
        )?;
        let revision = tx.query_row(
            "SELECT revision FROM auth_users WHERE id=?1 FOR UPDATE",
            [user.clone()],
            |r| r.get::<_, i64>(0),
        )?;
        let now = now_seconds();
        let (latest,count)=tx.query_row("SELECT MAX(COALESCE(activated_at,created_at)),COUNT(*) FROM auth_password_resets WHERE user_id=?1 AND COALESCE(activated_at,created_at)>?2",params![user,now-3600],|r|Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,i64>(1)?)))?;
        if let Some(latest) = latest
            && latest + PASSWORD_RESET_RESEND_SECONDS > now
        {
            return Err(AuthError::RateLimited {
                message: "重置邮件发送过于频繁，请稍后重试".into(),
                retry_after: latest + PASSWORD_RESET_RESEND_SECONDS - now,
            });
        }
        if count >= 5 {
            return Err(AuthError::RateLimited {
                message: "重置邮件请求次数过多，请稍后重试".into(),
                retry_after: 3600,
            });
        }
        let id = random_verification_id()?;
        let (token, _) = random_session_token()?;
        tx.execute("INSERT INTO auth_password_resets(id,user_id,token_digest,auth_revision,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6)",params![id,user,digest(&token)?,revision,now,now+PASSWORD_RESET_TTL_SECONDS])?;
        tx.commit()?;
        Ok(Some(IssuedPasswordReset { id, email, token }))
    }
    pub fn activate_password_reset(&self, id: &str) -> Result<(), AuthError> {
        let tx = self.connection()?.transaction()?;
        let user = tx
            .query_row(
                "SELECT user_id FROM auth_password_resets WHERE id=?1",
                [id],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(invalid)?;
        tx.query_row(
            "SELECT pg_advisory_xact_lock(hashtextextended(?1::TEXT,4271072007)),1::BIGINT",
            [user.clone()],
            |r| r.get::<_, i64>(1),
        )?;
        let revision = tx.query_row(
            "SELECT revision FROM auth_users WHERE id=?1 FOR UPDATE",
            [user.clone()],
            |r| r.get::<_, i64>(0),
        )?;
        let now = now_seconds();
        let activated=tx.execute("UPDATE auth_password_resets SET activated_at=?2,expires_at=?3 WHERE id=?1 AND auth_revision=?4 AND activated_at IS NULL AND consumed_at IS NULL AND expires_at>?2",params![id,now,now+PASSWORD_RESET_TTL_SECONDS,revision])?;
        if activated != 1 {
            return Err(invalid());
        }
        tx.execute("UPDATE auth_password_resets SET consumed_at=?3 WHERE user_id=?1 AND id<>?2 AND activated_at IS NOT NULL AND consumed_at IS NULL",params![user,id,now])?;
        tx.commit()?;
        Ok(())
    }
    pub fn discard_password_reset(&self, id: &str) -> Result<(), AuthError> {
        self.connection()?.execute(
            "DELETE FROM auth_password_resets WHERE id=?1 AND activated_at IS NULL",
            [id],
        )?;
        Ok(())
    }
    pub fn password_reset_valid_for(&self, token: &str) -> Result<i64, AuthError> {
        let expires=self.connection()?.query_row("SELECT p.expires_at FROM auth_password_resets p JOIN auth_users u ON u.id=p.user_id AND u.revision=p.auth_revision WHERE p.token_digest=?1 AND p.activated_at IS NOT NULL AND p.consumed_at IS NULL AND p.expires_at>?2",params![digest(token)?,now_seconds()],|r|r.get::<_,i64>(0)).optional()?.ok_or_else(invalid)?;
        Ok(expires.saturating_sub(now_seconds()))
    }
    pub fn complete_password_reset(&self, token: &str, password: &str) -> Result<(), AuthError> {
        self.password_reset_valid_for(token)?;
        let token = digest(token)?;
        validate_password(password)?;
        let hash = hash_password(password)?;
        let tx = self.connection()?.transaction()?;
        let user = tx
            .query_row(
                "SELECT user_id FROM auth_password_resets WHERE token_digest=?1",
                [token.clone()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(invalid)?;
        tx.query_row(
            "SELECT pg_advisory_xact_lock(hashtextextended(?1::TEXT,4271072007)),1::BIGINT",
            [user.clone()],
            |r| r.get::<_, i64>(1),
        )?;
        let revision = tx.query_row(
            "SELECT revision FROM auth_users WHERE id=?1 FOR UPDATE",
            [user.clone()],
            |r| r.get::<_, i64>(0),
        )?;
        let now = now_seconds();
        let consumed=tx.execute("UPDATE auth_password_resets SET consumed_at=?2 WHERE token_digest=?1 AND auth_revision=?3 AND activated_at IS NOT NULL AND consumed_at IS NULL AND expires_at>?2",params![token,now,revision])?;
        if consumed != 1 {
            return Err(invalid());
        }
        tx.execute(
            "UPDATE auth_users SET password_hash=?2,revision=revision+1,updated_at=?3 WHERE id=?1",
            params![user, hash, now],
        )?;
        tx.execute(
            "DELETE FROM auth_user_sessions WHERE user_id=?1",
            [user.clone()],
        )?;
        tx.execute("UPDATE auth_password_resets SET consumed_at=?2 WHERE user_id=?1 AND consumed_at IS NULL",params![user,now])?;
        tx.commit()?;
        Ok(())
    }
}
