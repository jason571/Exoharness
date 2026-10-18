// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Persistent identity, account grants and verification; no API/Agent dependency.
mod attempts;
pub mod external;
mod helpers;
pub mod mailer;
mod password_reset;
mod policy;
pub use password_reset::{
    IssuedPasswordReset, PASSWORD_RESET_RESEND_SECONDS, PASSWORD_RESET_TTL_SECONDS,
};
mod registration;
mod roles;
mod schema;
mod types;
mod user_directory;
mod users;
use exoharness_core::services::Service;
pub use exoharness_database::Database;
pub use schema::csrf_token;
pub use types::*;
impl Service for AuthStore {
    const KEY: &'static str = "identity";
}
#[cfg(test)]
#[path = "../tests/unit/legacy_identity.rs"]
mod tests;
