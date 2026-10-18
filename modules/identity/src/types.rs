// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_database::Database;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;
pub(crate) const MIN_PASSWORD_CHARS: usize = 8;
pub(crate) const MAX_PASSWORD_BYTES: usize = 256;
pub(crate) const SESSION_TOKEN_BYTES: usize = 32;
pub const DEFAULT_SESSION_TTL_SECONDS: i64 = 12 * 60 * 60;
pub(crate) const MIN_USERNAME_CHARS: usize = 3;
pub(crate) const MAX_USERNAME_CHARS: usize = 32;
pub(crate) const MAX_DISPLAY_NAME_CHARS: usize = 64;
pub(crate) const MAX_EMAIL_BYTES: usize = 254;
pub(crate) const REGISTRATION_CODE_DIGITS: usize = 6;
pub const REGISTRATION_CODE_TTL_SECONDS: i64 = 10 * 60;
pub(crate) const REGISTRATION_CODE_RESEND_SECONDS: i64 = 60;
pub(crate) const REGISTRATION_CODE_HOURLY_LIMIT: i64 = 5;
pub(crate) const REGISTRATION_CODE_ATTEMPTS: i64 = 5;
pub const DEFAULT_REGISTRATION_EMAIL_SUBJECT: &str = "ExoHarness 注册验证码";
pub const DEFAULT_REGISTRATION_EMAIL_BODY: &str = "你的 {{product_name}} 注册验证码是：{{code}}\n\n验证码 {{expires_minutes}} 分钟内有效，且只能使用一次。如果不是你本人操作，请忽略这封邮件。";
pub const DEFAULT_AGENT_PROFILE_ID: &str = "xiaolan";
pub(crate) const MAX_REGISTRATION_EMAIL_SUBJECT_CHARS: usize = 200;
pub(crate) const MAX_REGISTRATION_EMAIL_BODY_BYTES: usize = 20 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthRole {
    Admin,
    Chat,
}

impl AuthRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Admin => "admin",
            Self::Chat => "chat",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "admin" => Some(Self::Admin),
            "chat" => Some(Self::Chat),
            _ => None,
        }
    }
}

impl fmt::Display for AuthRole {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthConfiguration {
    pub admin_configured: bool,
    pub chat_configured: bool,
}

impl AuthConfiguration {
    pub fn any_configured(&self) -> bool {
        self.admin_configured || self.chat_configured
    }

    pub fn fully_configured(&self) -> bool {
        self.admin_configured && self.chat_configured
    }
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    pub token: String,
    pub role: AuthRole,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct ValidatedSession {
    pub role: AuthRole,
    pub expires_at: i64,
    pub token_digest: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct UserAccount {
    pub id: String,
    pub username: String,
    pub email: String,
    pub display_name: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// An account's category-level grants. Install and use are intentionally
/// independent: revoking installation must not delete an existing capability.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UserCapabilityPolicy {
    pub install_skills: bool,
    pub install_mcp: bool,
    pub install_tools: bool,
    pub use_skills: bool,
    pub use_mcp: bool,
    pub use_tools: bool,
}

impl Default for UserCapabilityPolicy {
    fn default() -> Self {
        Self {
            install_skills: false,
            install_mcp: false,
            install_tools: false,
            use_skills: true,
            use_mcp: true,
            use_tools: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityShareGrant {
    pub revision: i64,
    pub manifest_digest: String,
    pub source_digest: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserAuthSession {
    pub token: String,
    pub account: UserAccount,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct ValidatedUserSession {
    pub account: UserAccount,
    pub expires_at: i64,
    pub token_digest: String,
}

#[derive(Debug, Clone)]
pub struct IssuedRegistrationCode {
    pub id: String,
    pub email: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RegistrationEmailTemplate {
    pub subject: String,
    pub body: String,
    pub revision: i64,
    pub updated_at: Option<i64>,
}

impl RegistrationEmailTemplate {
    pub fn render(&self, code: &str) -> (String, String) {
        let expires_minutes = (REGISTRATION_CODE_TTL_SECONDS / 60).to_string();
        let render = |value: &str| {
            value
                .replace("{{code}}", code)
                .replace("{{expires_minutes}}", &expires_minutes)
                .replace("{{product_name}}", "ExoHarness")
        };
        (render(&self.subject), render(&self.body))
    }
}

impl Default for RegistrationEmailTemplate {
    fn default() -> Self {
        Self {
            subject: DEFAULT_REGISTRATION_EMAIL_SUBJECT.into(),
            body: DEFAULT_REGISTRATION_EMAIL_BODY.into(),
            revision: 0,
            updated_at: None,
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("password reset link is invalid, expired or consumed")]
    PasswordResetInvalid,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{message}")]
    RateLimited { message: String, retry_after: i64 },
    #[error("authentication storage failed: {0}")]
    Database(#[from] exoharness_database::Error),
    #[error("password hashing failed")]
    PasswordHash,
    #[error("secure random generation failed")]
    Random,
    #[error("registration email delivery failed")]
    EmailDelivery,
}

#[derive(Debug, Clone)]
pub struct AuthStore {
    pub(crate) database: Database,
}
