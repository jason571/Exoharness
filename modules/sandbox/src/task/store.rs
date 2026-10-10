// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{State, TaskSnapshot, files, revision_path};
use crate::{Result, SandboxError, filesystem};
use fs2::FileExt;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Clone)]
pub struct TaskStore {
    pub(super) root: PathBuf,
}
pub(super) struct TaskLease(fs::File);
impl Drop for TaskLease {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
#[cfg(test)]
#[path = "../../tests/unit/task_lease.rs"]
mod lease_tests;
impl TaskStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        filesystem::private(&root)?;
        filesystem::private(&root.join("tasks"))?;
        filesystem::private(&root.join("workspaces"))?;
        filesystem::private(&root.join("jobs"))?;
        Ok(Self { root })
    }
    pub fn create(
        &self,
        owner: &str,
        session: &str,
        operation: &str,
        skill: Option<(String, i64)>,
    ) -> Result<TaskSnapshot> {
        for id in [owner, session, operation] {
            filesystem::identity(id)?;
        }
        let id = filesystem::digest(
            &serde_json::to_string(&(owner, session, operation))
                .map_err(|_| SandboxError::Invalid)?,
        );
        let directory = self.root.join("tasks").join(&id);
        filesystem::private(&directory)?;
        let _lock = self.lock(&directory)?;
        if directory.join("state.json").exists() {
            return Ok(self.load(&directory, owner, session)?.snapshot);
        }
        filesystem::private(&revision_path(&directory, 1))?;
        let snapshot = TaskSnapshot {
            workspace_id: id,
            revision: 1,
            files: vec![],
            skill_id: skill.as_ref().map(|v| v.0.clone()),
            skill_revision: skill.map(|v| v.1),
            notes: String::new(),
            running_invocation: None,
            last_job: None,
        };
        self.save(
            &directory,
            &State {
                owner: owner.into(),
                session: session.into(),
                snapshot: snapshot.clone(),
                pending: None,
                operations: BTreeMap::new(),
            },
        )?;
        Ok(snapshot)
    }
    pub fn inspect(&self, owner: &str, session: &str, id: &str) -> Result<TaskSnapshot> {
        let directory = self.directory(id)?;
        let _lock = self.lock(&directory)?;
        Ok(self.reconcile(&directory, owner, session)?.snapshot)
    }
    pub(super) fn directory(&self, id: &str) -> Result<PathBuf> {
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(SandboxError::Invalid);
        }
        let directory = self.root.join("tasks").join(id);
        if !directory.try_exists().map_err(|_| SandboxError::Storage)? {
            return Err(SandboxError::Invalid);
        }
        filesystem::private(&directory)?;
        Ok(directory)
    }
    pub(super) fn lock(&self, directory: &Path) -> Result<TaskLease> {
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let path = directory.join("task.lock");
        if path.exists()
            && fs::symlink_metadata(&path)
                .map_err(|_| SandboxError::Storage)?
                .file_type()
                .is_symlink()
        {
            return Err(SandboxError::Invalid);
        }
        let file = options.open(path).map_err(|_| SandboxError::Storage)?;
        file.try_lock_exclusive()
            .map_err(|_| SandboxError::WorkspaceBusy)?;
        Ok(TaskLease(file))
    }
    pub(super) fn load(&self, directory: &Path, owner: &str, session: &str) -> Result<State> {
        let bytes = fs::read(directory.join("state.json")).map_err(|_| SandboxError::Storage)?;
        let state: State = serde_json::from_slice(&bytes).map_err(|_| SandboxError::Storage)?;
        if state.owner != owner || state.session != session {
            return Err(SandboxError::Invalid);
        }
        Ok(state)
    }
    pub(super) fn save(&self, directory: &Path, state: &State) -> Result<()> {
        let mut file =
            tempfile::NamedTempFile::new_in(directory).map_err(|_| SandboxError::Storage)?;
        file.write_all(&serde_json::to_vec(state).map_err(|_| SandboxError::Storage)?)
            .and_then(|_| file.as_file().sync_all())
            .map_err(|_| SandboxError::Storage)?;
        file.persist(directory.join("state.json"))
            .map_err(|_| SandboxError::Storage)?;
        fs::File::open(directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| SandboxError::Storage)
    }
    pub(super) fn install(
        &self,
        directory: &Path,
        state: &mut State,
        source: &Path,
        operation: &str,
    ) -> Result<()> {
        let revision = state
            .snapshot
            .revision
            .checked_add(1)
            .ok_or(SandboxError::Invalid)?;
        let target = revision_path(directory, revision);
        // An interrupted commit is never silently overwritten.
        if target.exists() {
            let candidate = files::inventory(source)?;
            let retained = files::inventory(&target)?;
            if serde_json::to_value(&candidate).map_err(|_| SandboxError::Storage)?
                != serde_json::to_value(&retained).map_err(|_| SandboxError::Storage)?
            {
                return Err(SandboxError::OutcomeUnknown);
            }
            state.snapshot.revision = revision;
            state.snapshot.files = retained;
            state.operations.insert(operation.into(), revision);
            state.pending = None;
            state.snapshot.running_invocation = None;
            return self.save(directory, state);
        }
        let staged = files::stage(directory)?;
        files::copy(source, staged.path())?;
        let inventory = files::inventory(staged.path())?;
        fs::rename(staged.path(), &target).map_err(|_| SandboxError::Storage)?;
        state.snapshot.revision = revision;
        state.snapshot.files = inventory;
        state.operations.insert(operation.into(), revision);
        state.pending = None;
        state.snapshot.running_invocation = None;
        self.save(directory, state)
    }
    pub fn list(&self, owner: &str, session: &str) -> Result<Vec<TaskSnapshot>> {
        let mut tasks = Vec::new();
        for entry in fs::read_dir(self.root.join("tasks")).map_err(|_| SandboxError::Storage)? {
            let entry = entry.map_err(|_| SandboxError::Storage)?;
            if !entry
                .file_type()
                .map_err(|_| SandboxError::Storage)?
                .is_dir()
            {
                continue;
            }
            let state = match self.load(&entry.path(), owner, session) {
                Ok(state) => state,
                Err(SandboxError::Invalid) => continue,
                Err(error) => return Err(error),
            };
            tasks.push(state.snapshot);
            if tasks.len() >= 100 {
                break;
            }
        }
        tasks.sort_by(|a, b| a.workspace_id.cmp(&b.workspace_id));
        Ok(tasks)
    }
    pub fn job(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        invocation: &str,
    ) -> Result<Option<crate::Receipt>> {
        let directory = self.directory(id)?;
        let state = self.load(&directory, owner, session)?;
        if !state.operations.contains_key(invocation)
            && state.pending.as_ref().map(|p| p.invocation.as_str()) != Some(invocation)
        {
            return Err(SandboxError::Invalid);
        }
        filesystem::receipt(&self.root, invocation)
    }
}
