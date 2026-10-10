// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{Assistants, ProfileState, persistence};
use crate::{ConfigurationError, Result};
use serde_json::{Value, json};
use std::{fs::File, sync::MutexGuard};
/// Held within a blocking assignment transaction, never across async waits.
pub struct AssignmentLease<'a> {
    _state: MutexGuard<'a, ProfileState>,
    _file: Option<File>,
    profile: Value,
}
impl AssignmentLease<'_> {
    pub fn profile(&self) -> &Value {
        &self.profile
    }
}
impl Assistants {
    pub fn assignment(&self, id: &str) -> Result<AssignmentLease<'_>> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ConfigurationError::PrivateFile)?;
        let file = if let Some(root) = &self.root {
            let file = persistence::lock(root)?;
            state.catalog = persistence::load(root)?;
            Some(file)
        } else {
            None
        };
        let record = state.catalog["profiles"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["id"].as_str() == Some(id))
            .ok_or(ConfigurationError::NotFound)?;
        let profile = record["revisions"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|r| r["revision"] == record["active_revision"])
            .ok_or(ConfigurationError::NotFound)?;
        let default = state.catalog["default_profile"].as_str() == Some(id);
        if record["enabled"] != true
            || (!default && profile.pointer("/access/published") != Some(&json!(true)))
        {
            return Err(ConfigurationError::Invalid(
                "assistant must be enabled and published",
            ));
        }
        let mut profile = profile.clone();
        profile["enabled"] = json!(true);
        profile["is_default"] = json!(default);
        Ok(AssignmentLease {
            _state: state,
            _file: file,
            profile,
        })
    }
}
