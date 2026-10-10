// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Deployment configuration, not an Agent, model adapter or global state bag.
//! Import/read never calls a provider, compacts an assistant, or grants tools.
mod files;
mod models;
mod profiles;
mod validation;
pub use files::selected_generation;
pub use models::{ModelBinding, Models};
pub use profiles::{AssignmentLease, Assistants};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("private configuration I/O or permissions are invalid")]
    PrivateFile,
    #[error("configuration JSON is invalid or has duplicate keys")]
    Json,
    #[error("configuration does not match the declared contract: {0}")]
    Invalid(&'static str),
    #[error("configuration item was not found")]
    NotFound,
    #[error("configuration changed; refresh the current revision")]
    Conflict,
}
type Result<T> = std::result::Result<T, ConfigurationError>;

pub fn check_directory(root: &Path) -> Result<serde_json::Value> {
    let models = Models::open(root)?;
    let assistants = Assistants::open(root)?;
    Ok(
        serde_json::json!({"models":models.catalog()["models"].as_array().map_or(0,Vec::len),
        "providers":models.catalog()["providers"].as_array().map_or(0,Vec::len),
        "credentials":models.credential_count()?, "assistants":assistants.list().len(),
        "avatars":assistants.avatar_count(), "runtime_ready":false}),
    )
}
