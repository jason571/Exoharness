// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Assistant snapshots; reading does not grant invocation authority.
mod access;
mod assignment;
mod avatars;
mod mutations;
mod persistence;
mod projection;
mod validation;
use crate::{Result, files};
pub use assignment::AssignmentLease;
use exoharness_core::services::Service;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Original revisions and selections are preserved, not silently compacted
/// on read. Selection is configuration; invocation authorization is separate.
pub struct Assistants {
    state: Mutex<ProfileState>,
    root: Option<PathBuf>,
}
struct ProfileState {
    catalog: Value,
    avatars: BTreeMap<String, (String, Vec<u8>)>,
}
impl Service for Assistants {
    const KEY: &'static str = "assistant-configuration";
}
impl Assistants {
    pub fn empty() -> Self {
        Self {
            state: Mutex::new(ProfileState {
                catalog: json!({"profiles":[]}),
                avatars: BTreeMap::new(),
            }),
            root: None,
        }
    }
    pub fn open(root: &Path) -> Result<Self> {
        files::private(root, true)?;
        let catalog = persistence::load(root)?;
        let avatars = avatars::load(root, &catalog)?;
        Ok(Self {
            state: Mutex::new(ProfileState { catalog, avatars }),
            root: Some(root.into()),
        })
    }
    pub fn initialize(root: &Path) -> Result<Self> {
        persistence::initialize(root)?;
        Self::open(root)
    }
    fn catalog(&self) -> Value {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .catalog
            .clone()
    }
    pub fn refresh(&self) -> Result<()> {
        if let Some(root) = &self.root {
            let mut state = self
                .state
                .lock()
                .map_err(|_| crate::ConfigurationError::PrivateFile)?;
            let catalog = persistence::load(root)?;
            if catalog == state.catalog {
                return Ok(());
            }
            let avatars = avatars::load(root, &catalog)?;
            *state = ProfileState { catalog, avatars };
        }
        Ok(())
    }
}
