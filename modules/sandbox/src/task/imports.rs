// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{TaskSnapshot, TaskStore, files, revision_path};
use crate::{Result, SandboxError, filesystem};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
};
impl TaskStore {
    pub fn import(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        revision: u64,
        operation: &str,
        inputs: BTreeMap<String, fs::File>,
    ) -> Result<TaskSnapshot> {
        filesystem::identity(operation)?;
        if inputs.is_empty() || inputs.len() > 12 {
            return Err(SandboxError::Invalid);
        }
        let directory = self.directory(id)?;
        let _lock = self.lock(&directory)?;
        let mut state = self.reconcile(&directory, owner, session)?;
        if state.operations.contains_key(operation) {
            return Ok(state.snapshot);
        }
        if state.pending.is_some() {
            return Err(SandboxError::WorkspaceBusy);
        }
        if state.snapshot.revision != revision {
            return Err(SandboxError::StaleRevision);
        }
        let existing = state
            .snapshot
            .files
            .iter()
            .map(|f| f.path.as_str())
            .collect::<BTreeSet<_>>();
        for path in inputs.keys() {
            filesystem::relative(path)?;
            if path.split('/').next() == Some("skill") || existing.contains(path.as_str()) {
                return Err(SandboxError::Invalid);
            }
        }
        let staged = files::stage(&directory)?;
        files::copy(&revision_path(&directory, revision), staged.path())?;
        for (path, file) in inputs {
            files::write_parent(staged.path(), &path)?;
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(staged.path().join(path))
                .map_err(|_| SandboxError::Storage)?;
            let bytes = std::io::copy(&mut file.take(128 * 1024 * 1024 + 1), &mut out)
                .map_err(|_| SandboxError::Storage)?;
            if bytes > 128 * 1024 * 1024 {
                return Err(SandboxError::Invalid);
            }
            out.sync_all().map_err(|_| SandboxError::Storage)?;
        }
        self.install(&directory, &mut state, staged.path(), operation)?;
        Ok(state.snapshot)
    }
}
