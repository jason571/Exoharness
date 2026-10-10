//! Persistent identity, account grants and verification; no API/Agent dependency.
mod attempts;
mod helpers;
pub mod mailer;
mod policy;
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
