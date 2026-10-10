// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::types::{
    AuthError, AuthRole, MAX_DISPLAY_NAME_CHARS, MAX_EMAIL_BYTES, MAX_PASSWORD_BYTES,
    MAX_REGISTRATION_EMAIL_BODY_BYTES, MAX_REGISTRATION_EMAIL_SUBJECT_CHARS, MAX_USERNAME_CHARS,
    MIN_PASSWORD_CHARS, MIN_USERNAME_CHARS, SESSION_TOKEN_BYTES,
};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn token_digest(token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(b"exoharness-session-v1\0");
    digest.update(token.as_bytes());
    hex_bytes(&digest.finalize())
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(crate) fn opposite_role(role: AuthRole) -> AuthRole {
    match role {
        AuthRole::Admin => AuthRole::Chat,
        AuthRole::Chat => AuthRole::Admin,
    }
}

pub(crate) fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(AuthError::Validation(format!(
            "密码至少需要 {MIN_PASSWORD_CHARS} 个字符"
        )));
    }
    if password.len() > MAX_PASSWORD_BYTES {
        return Err(AuthError::Validation(format!(
            "密码不能超过 {MAX_PASSWORD_BYTES} 字节"
        )));
    }
    Ok(())
}

pub(crate) fn validate_registration_email_template(
    subject: &str,
    body: &str,
) -> Result<(String, String), AuthError> {
    let subject = subject.trim();
    if subject.is_empty() {
        return Err(AuthError::Validation("邮件标题不能为空".into()));
    }
    if subject.contains(['\r', '\n']) {
        return Err(AuthError::Validation("邮件标题只能使用一行文字".into()));
    }
    if subject.chars().count() > MAX_REGISTRATION_EMAIL_SUBJECT_CHARS {
        return Err(AuthError::Validation(format!(
            "邮件标题不能超过 {MAX_REGISTRATION_EMAIL_SUBJECT_CHARS} 个字符"
        )));
    }
    let body = body.replace("\r\n", "\n").replace('\r', "\n");
    if body.trim().is_empty() {
        return Err(AuthError::Validation("邮件正文不能为空".into()));
    }
    if body.len() > MAX_REGISTRATION_EMAIL_BODY_BYTES {
        return Err(AuthError::Validation(format!(
            "邮件正文不能超过 {MAX_REGISTRATION_EMAIL_BODY_BYTES} 字节"
        )));
    }
    if !body.contains("{{code}}") {
        return Err(AuthError::Validation(
            "邮件正文必须包含验证码占位符 {{code}}".into(),
        ));
    }
    for value in [subject, body.as_str()] {
        let mut remaining = value;
        while let Some(start) = remaining.find("{{") {
            let after_start = &remaining[start + 2..];
            let Some(end) = after_start.find("}}") else {
                return Err(AuthError::Validation("邮件模板包含未闭合的占位符".into()));
            };
            let placeholder = &after_start[..end];
            if !matches!(placeholder, "code" | "expires_minutes" | "product_name") {
                return Err(AuthError::Validation(format!(
                    "不支持邮件占位符 {{{{{placeholder}}}}}"
                )));
            }
            remaining = &after_start[end + 2..];
        }
    }
    Ok((subject.to_owned(), body))
}

pub(crate) fn validate_username(username: &str) -> Result<String, AuthError> {
    let username = username.trim();
    let count = username.chars().count();
    if !(MIN_USERNAME_CHARS..=MAX_USERNAME_CHARS).contains(&count)
        || !username
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(AuthError::Validation(format!(
            "用户名需要 {MIN_USERNAME_CHARS}-{MAX_USERNAME_CHARS} 位，只能包含字母、数字、点、下划线或短横线"
        )));
    }
    Ok(username.to_owned())
}

pub(crate) fn validate_display_name(display_name: &str) -> Result<String, AuthError> {
    let display_name = display_name.trim();
    if display_name.is_empty()
        || display_name.chars().count() > MAX_DISPLAY_NAME_CHARS
        || display_name.chars().any(char::is_control)
    {
        return Err(AuthError::Validation(format!(
            "显示名称需要 1-{MAX_DISPLAY_NAME_CHARS} 个可见字符"
        )));
    }
    Ok(display_name.to_owned())
}

pub(crate) fn validate_email(email: &str) -> Result<(String, String), AuthError> {
    let email = email.trim();
    if email.is_empty()
        || email.len() > MAX_EMAIL_BYTES
        || !email.is_ascii()
        || email
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
    {
        return Err(AuthError::Validation("请输入有效的邮箱地址".into()));
    }
    let mut parts = email.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty()
        || local.len() > 64
        || domain.is_empty()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || parts.next().is_some()
    {
        return Err(AuthError::Validation("请输入有效的邮箱地址".into()));
    }
    Ok((email.to_owned(), email.to_ascii_lowercase()))
}

pub(crate) fn random_user_id() -> Result<String, AuthError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::Random)?;
    Ok(format!("user_{}", hex_bytes(&bytes)))
}

pub(crate) fn random_verification_id() -> Result<String, AuthError> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::Random)?;
    Ok(format!("verify_{}", hex_bytes(&bytes)))
}

pub(crate) fn random_registration_code() -> Result<String, AuthError> {
    const RANGE: u32 = 1_000_000;
    const ACCEPT_BELOW: u32 = u32::MAX - (u32::MAX % RANGE);
    loop {
        let mut bytes = [0_u8; 4];
        getrandom::fill(&mut bytes).map_err(|_| AuthError::Random)?;
        let value = u32::from_le_bytes(bytes);
        if value < ACCEPT_BELOW {
            return Ok(format!("{:06}", value % RANGE));
        }
    }
}

pub(crate) fn random_session_token() -> Result<(String, String), AuthError> {
    let mut bytes = [0_u8; SESSION_TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::Random)?;
    let token = URL_SAFE_NO_PAD.encode(bytes);
    let digest = token_digest(&token);
    Ok((token, digest))
}

pub(crate) fn argon2() -> Result<Argon2<'static>, AuthError> {
    let params = Params::new(19 * 1024, 2, 1, None).map_err(|_| AuthError::PasswordHash)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub(crate) fn hash_password(password: &str) -> Result<String, AuthError> {
    let mut salt_bytes = [0_u8; 16];
    getrandom::fill(&mut salt_bytes).map_err(|_| AuthError::Random)?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|_| AuthError::PasswordHash)?;
    argon2()?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| AuthError::PasswordHash)
}

pub(crate) fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded).ok().is_some_and(|hash| {
        argon2().is_ok_and(|hasher| hasher.verify_password(password.as_bytes(), &hash).is_ok())
    })
}

pub(crate) fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .try_into()
        .unwrap_or(i64::MAX)
}
