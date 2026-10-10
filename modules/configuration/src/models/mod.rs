// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Versioned model catalog; validation, persistence and secrets have separate owners.
mod catalog;
mod credentials;
mod defaults;
mod kinds;
mod mutations;
mod parameters;
mod persistence;
mod providers;
mod runtime;
mod selection;
mod speech;
use crate::{Result, files};
use exoharness_core::services::Service;
pub use runtime::ModelBinding;
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

pub struct Models {
    state: Mutex<persistence::Snapshot>,
    root: Option<PathBuf>,
}
impl Service for Models {
    const KEY: &'static str = "model-configuration";
}
impl Models {
    pub fn empty() -> Self {
        Self {
            state: Mutex::new(persistence::Snapshot {
                catalog: serde_json::json!({"providers":[],"models":[],"default_model":null,"default_models":{}}),
                root: None,
                revision: 0,
            }),
            root: None,
        }
    }
    pub fn open(root: &Path) -> Result<Self> {
        files::private(root, true)?;
        Ok(Self {
            state: Mutex::new(persistence::load(root)?),
            root: Some(root.into()),
        })
    }
    pub fn initialize(root: &Path) -> Result<Self> {
        persistence::initialize(root)?;
        Self::open(root)
    }
    pub fn catalog(&self) -> Value {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .catalog
            .clone()
    }
    pub fn credential_count(&self) -> Result<usize> {
        let state = self
            .state
            .lock()
            .map_err(|_| crate::ConfigurationError::PrivateFile)?;
        credentials::count(state.root.as_deref(), &state.catalog)
    }
    pub fn revision(&self) -> u64 {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .revision
    }
    /// Rebuild the public projection from durable truth after an ambiguous
    /// write receipt or a second authorised writer. Never mutate old snapshots.
    pub fn refresh(&self) -> Result<Value> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| crate::ConfigurationError::PrivateFile)?;
        if let Some(root) = &self.root {
            *state = persistence::load(root)?;
        }
        Ok(state.catalog.clone())
    }
}
