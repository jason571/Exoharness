use crate::helpers::{
    hash_password, now_seconds, random_registration_code, random_session_token, random_user_id,
    random_verification_id, validate_display_name, validate_email, validate_password,
    validate_registration_email_template, validate_username, verify_password,
};
use crate::types::AuthStore;
use crate::types::{
    AuthError, DEFAULT_AGENT_PROFILE_ID, IssuedRegistrationCode, REGISTRATION_CODE_ATTEMPTS,
    REGISTRATION_CODE_DIGITS, REGISTRATION_CODE_HOURLY_LIMIT, REGISTRATION_CODE_RESEND_SECONDS,
    REGISTRATION_CODE_TTL_SECONDS, RegistrationEmailTemplate, UserAccount, UserAuthSession,
};
use exoharness_database::OptionalExtension;
use exoharness_database::params;
impl AuthStore {
    pub fn registration_email_template(&self) -> Result<RegistrationEmailTemplate, AuthError> {
        Ok(self
            .connection()?
            .query_row(
                "SELECT subject_template, body_template, revision, updated_at
                 FROM auth_registration_email_settings WHERE singleton_id=1",
                (),
                |row| {
                    Ok(RegistrationEmailTemplate {
                        subject: row.get(0)?,
                        body: row.get(1)?,
                        revision: row.get(2)?,
                        updated_at: Some(row.get(3)?),
                    })
                },
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn update_registration_email_template(
        &self,
        subject: &str,
        body: &str,
        expected_revision: i64,
    ) -> Result<RegistrationEmailTemplate, AuthError> {
        let (subject, body) = validate_registration_email_template(subject, body)?;
        if expected_revision < 0 {
            return Err(AuthError::Validation("邮件模板 revision 不能小于 0".into()));
        }
        let now = now_seconds();
        let transaction = self.connection()?.transaction()?;
        transaction.query_row(
            "SELECT pg_advisory_xact_lock(hashtext('auth_registration_email_settings')), 1::BIGINT",
            (),
            |row| row.get::<_, i64>(1),
        )?;
        let current_revision = transaction
            .query_row(
                "SELECT revision FROM auth_registration_email_settings WHERE singleton_id=1",
                (),
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0);
        if current_revision != expected_revision {
            return Err(AuthError::Conflict(format!(
                "邮件模板已更新，请刷新后重试（当前 revision {current_revision}）"
            )));
        }
        let revision = current_revision.saturating_add(1);
        transaction.execute(
            "INSERT INTO auth_registration_email_settings(
                 singleton_id, subject_template, body_template, revision, updated_at
             ) VALUES(1, ?1, ?2, ?3, ?4)
             ON CONFLICT(singleton_id) DO UPDATE SET
                 subject_template=excluded.subject_template,
                 body_template=excluded.body_template,
                 revision=excluded.revision,
                 updated_at=excluded.updated_at",
            params![subject, body, revision, now],
        )?;
        transaction.commit()?;
        Ok(RegistrationEmailTemplate {
            subject,
            body,
            revision,
            updated_at: Some(now),
        })
    }

    pub fn issue_registration_code(
        &self,
        email: &str,
    ) -> Result<Option<IssuedRegistrationCode>, AuthError> {
        let (email, normalized_email) = validate_email(email)?;
        let now = now_seconds();
        let transaction = self.connection()?.transaction()?;
        transaction.query_row(
            "SELECT pg_advisory_xact_lock(hashtext(?1)), 1::BIGINT",
            params![normalized_email],
            |row| row.get::<_, i64>(1),
        )?;
        let already_registered = transaction
            .query_row(
                "SELECT TRUE FROM auth_users WHERE normalized_email=?1",
                params![normalized_email],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
            .is_some();
        if already_registered {
            transaction.commit()?;
            return Ok(None);
        }
        let (latest_sent_at, sent_this_hour) = transaction.query_row(
            "SELECT MAX(COALESCE(activated_at, created_at)), COUNT(*)
             FROM auth_email_verifications
             WHERE normalized_email=?1
               AND COALESCE(activated_at, created_at)>?2",
            params![normalized_email, now.saturating_sub(60 * 60)],
            |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?)),
        )?;
        if let Some(latest_sent_at) = latest_sent_at
            && latest_sent_at.saturating_add(REGISTRATION_CODE_RESEND_SECONDS) > now
        {
            return Err(AuthError::RateLimited {
                message: "验证码发送过于频繁，请稍后再试".into(),
                retry_after: latest_sent_at
                    .saturating_add(REGISTRATION_CODE_RESEND_SECONDS)
                    .saturating_sub(now),
            });
        }
        if sent_this_hour >= REGISTRATION_CODE_HOURLY_LIMIT {
            return Err(AuthError::RateLimited {
                message: "该邮箱请求验证码次数过多，请稍后再试".into(),
                retry_after: 60 * 60,
            });
        }
        let id = random_verification_id()?;
        let code = random_registration_code()?;
        let code_hash = hash_password(&code)?;
        transaction.execute(
            "INSERT INTO auth_email_verifications(
                 id, email, normalized_email, code_hash, created_at, expires_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                id,
                email,
                normalized_email,
                code_hash,
                now,
                now.saturating_add(REGISTRATION_CODE_TTL_SECONDS),
            ],
        )?;
        transaction.commit()?;
        Ok(Some(IssuedRegistrationCode { id, email, code }))
    }

    pub fn activate_registration_code(&self, id: &str) -> Result<(), AuthError> {
        let now = now_seconds();
        let transaction = self.connection()?.transaction()?;
        let verification = transaction
            .query_row(
                "SELECT normalized_email FROM auth_email_verifications
                 WHERE id=?1 AND activated_at IS NULL AND consumed_at IS NULL AND expires_at>?2",
                params![id, now],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(normalized_email) = verification else {
            return Err(AuthError::Validation(
                "registration verification could not be activated".into(),
            ));
        };
        transaction.execute(
            "UPDATE auth_email_verifications SET consumed_at=?3
             WHERE normalized_email=?1 AND id<>?2 AND activated_at IS NOT NULL
               AND consumed_at IS NULL",
            params![normalized_email, id, now],
        )?;
        transaction.execute(
            "UPDATE auth_email_verifications SET activated_at=?2 WHERE id=?1",
            params![id, now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn discard_registration_code(&self, id: &str) -> Result<(), AuthError> {
        self.connection()?.execute(
            "DELETE FROM auth_email_verifications WHERE id=?1 AND activated_at IS NULL",
            params![id],
        )?;
        Ok(())
    }

    pub fn register_verified_user(
        &self,
        email: &str,
        verification_code: &str,
        username: &str,
        display_name: &str,
        password: &str,
        ttl_seconds: i64,
    ) -> Result<UserAuthSession, AuthError> {
        let (email, normalized_email) = validate_email(email)?;
        let verification_code = verification_code.trim();
        if verification_code.len() != REGISTRATION_CODE_DIGITS
            || !verification_code.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(AuthError::Validation("邮箱验证码不正确或已过期".into()));
        }
        let username = validate_username(username)?;
        let normalized_username = username.to_ascii_lowercase();
        let display_name = validate_display_name(display_name)?;
        validate_password(password)?;
        let password_hash = hash_password(password)?;
        let id = random_user_id()?;
        let (token, token_digest) = random_session_token()?;
        let now = now_seconds();
        let expires_at = now.saturating_add(ttl_seconds);
        let transaction = self.connection()?.transaction()?;
        let verification = transaction
            .query_row(
                "SELECT id, code_hash, expires_at, attempt_count
                 FROM auth_email_verifications
                 WHERE normalized_email=?1 AND activated_at IS NOT NULL AND consumed_at IS NULL
                 ORDER BY activated_at DESC LIMIT 1 FOR UPDATE",
                params![normalized_email],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((verification_id, code_hash, code_expires_at, attempt_count)) = verification
        else {
            return Err(AuthError::Validation("邮箱验证码不正确或已过期".into()));
        };
        if code_expires_at <= now || attempt_count >= REGISTRATION_CODE_ATTEMPTS {
            transaction.execute(
                "UPDATE auth_email_verifications SET consumed_at=?2 WHERE id=?1",
                params![verification_id, now],
            )?;
            transaction.commit()?;
            return Err(AuthError::Validation("邮箱验证码不正确或已过期".into()));
        }
        transaction.execute(
            "UPDATE auth_email_verifications SET attempt_count=attempt_count+1 WHERE id=?1",
            params![verification_id],
        )?;
        if !verify_password(verification_code, &code_hash) {
            transaction.commit()?;
            return Err(AuthError::Validation("邮箱验证码不正确或已过期".into()));
        }
        let inserted = transaction.execute(
            "INSERT INTO auth_users(
                 id, username, normalized_username, email, normalized_email,
                 display_name, password_hash, default_agent_profile_id,
                 revision, created_at, updated_at
             ) VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9)
             ON CONFLICT DO NOTHING",
            params![
                id,
                username,
                normalized_username,
                email,
                normalized_email,
                display_name,
                password_hash,
                DEFAULT_AGENT_PROFILE_ID,
                now,
            ],
        )?;
        if inserted != 1 {
            let email_exists = transaction
                .query_row(
                    "SELECT TRUE FROM auth_users WHERE normalized_email=?1",
                    params![normalized_email],
                    |row| row.get::<_, bool>(0),
                )
                .optional()?
                .is_some();
            return Err(AuthError::Conflict(if email_exists {
                "邮箱已经注册".into()
            } else {
                "用户名已经注册".into()
            }));
        }
        transaction.execute(
            "UPDATE auth_email_verifications SET consumed_at=?2 WHERE id=?1",
            params![verification_id, now],
        )?;
        transaction.execute(
            "INSERT INTO auth_user_sessions(
                 token_digest, user_id, auth_revision, created_at, expires_at
             ) VALUES(?1, ?2, 1, ?3, ?4)",
            params![token_digest, id, now, expires_at],
        )?;
        transaction.commit()?;
        Ok(UserAuthSession {
            token,
            account: UserAccount {
                id,
                username,
                email,
                display_name,
                created_at: now,
                updated_at: now,
            },
            expires_at,
        })
    }
}
