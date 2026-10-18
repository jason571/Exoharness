// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Confidential application delegation. Opaque credentials remain separate
//! from browser sessions; device and business identities belong to the caller.
mod applications;
mod backend;
mod grants;
mod tokens;
mod validation;
use crate::{AuthError, AuthStore};
use serde::{Deserialize, Serialize};

pub const SCOPES: &[&str] = &[
    "identity:read",
    "sessions:read",
    "sessions:write",
    "runs:read",
    "runs:cancel",
];
pub const ACCESS_TTL_SECONDS: i64 = 300;
pub const GRANT_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationInput {
    pub client_id: String,
    pub name: String,
    pub assistant_id: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    #[serde(default = "enabled")]
    pub enabled: bool,
    #[serde(default = "access_ttl")]
    pub access_ttl_seconds: i64,
    #[serde(default = "grant_ttl")]
    pub grant_ttl_seconds: i64,
    #[serde(default)]
    pub require_device_id: bool,
    #[serde(default = "enabled")]
    pub allow_backend_identity: bool,
}
fn enabled() -> bool {
    true
}
fn access_ttl() -> i64 {
    ACCESS_TTL_SECONDS
}
fn grant_ttl() -> i64 {
    GRANT_TTL_SECONDS
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Application {
    #[serde(flatten)]
    pub input: ApplicationInput,
    pub revision: i64,
}
// Never implement Debug for raw credentials.
#[derive(Serialize)]
pub struct IssuedApplication {
    pub application: Application,
    pub client_secret: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorizationRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub response_type: String,
}
#[derive(Clone, Serialize)]
pub struct Grant {
    pub id: String,
    pub client_id: String,
    pub user_id: String,
    pub assistant_id: String,
    pub scopes: Vec<String>,
    pub expires_at: i64,
    pub require_device_id: bool,
    pub device_id: Option<String>,
}
#[derive(Serialize)]
pub struct IssuedTokens {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: String,
    pub scope: String,
    pub user_id: String,
    pub grant_id: String,
}

pub(crate) fn migrate(store: &AuthStore) -> Result<(), AuthError> {
    let tx = store.connection()?.transaction()?;
    tx.execute(
        "SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),4271072002))",
        (),
    )?;
    tx.execute_batch(include_str!(
        "../../migrations/0001_external_applications.sql"
    ))?;
    tx.execute_batch(include_str!(
        "../../migrations/0002_external_key_storage.sql"
    ))?;
    tx.execute_batch(include_str!(
        "../../migrations/0003_external_backend_identity.sql"
    ))?;
    tx.execute_batch(include_str!(
        "../../migrations/0004_external_application_deletion.sql"
    ))?;
    tx.commit()?;
    Ok(())
}
pub use validation::pkce_challenge;
