use super::{Pending, State, TaskSnapshot, TaskStore, execution_workspace, files, revision_path};
use crate::{Receipt, Result, SandboxError, Workspace, filesystem};
use std::path::Path;
pub struct TaskExecution {
    pub workspace: Workspace,
    pub snapshot: TaskSnapshot,
    store: TaskStore,
    owner: String,
    session: String,
    operation: String,
    directory: std::path::PathBuf,
    // Keep writes, publication and another execution out until this call ends.
    _lock: super::store::TaskLease,
}
impl TaskStore {
    pub fn begin(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        revision: u64,
        operation: &str,
    ) -> Result<TaskExecution> {
        filesystem::identity(operation)?;
        let directory = self.directory(id)?;
        let lock = self.lock(&directory)?;
        let mut state = self.reconcile(&directory, owner, session)?;
        if state.pending.is_some() {
            return Err(SandboxError::WorkspaceBusy);
        }
        if state.operations.contains_key(operation) {
            return Err(SandboxError::OutcomeUnknown);
        }
        if state.snapshot.revision != revision {
            return Err(SandboxError::StaleRevision);
        }
        let job_id = filesystem::digest(operation);
        let target = self.root.join("workspaces").join(&job_id);
        if target.exists() {
            return Err(SandboxError::OutcomeUnknown);
        }
        filesystem::private(&target)?;
        files::copy(&revision_path(&directory, revision), &target)?;
        state.pending = Some(Pending {
            invocation: operation.into(),
            workspace: job_id.clone(),
        });
        state.snapshot.running_invocation = Some(operation.into());
        self.save(&directory, &state)?;
        Ok(TaskExecution {
            workspace: execution_workspace(self, &job_id),
            snapshot: state.snapshot,
            store: self.clone(),
            owner: owner.into(),
            session: session.into(),
            operation: operation.into(),
            directory,
            _lock: lock,
        })
    }
    pub(super) fn reconcile(&self, directory: &Path, owner: &str, session: &str) -> Result<State> {
        let mut state = self.load(directory, owner, session)?;
        if let Some(pending) = state.pending.clone()
            && let Some(receipt) = filesystem::receipt(&self.root, &pending.invocation)?
        {
            if !receipt.cleanup_confirmed {
                return Err(SandboxError::CleanupPending);
            }
            let work = self.root.join("workspaces").join(pending.workspace);
            state.snapshot.last_job = Some(super::TaskJob {
                invocation_id: receipt.invocation_id.clone(),
                status: receipt.status.clone(),
                cleanup_confirmed: receipt.cleanup_confirmed,
            });
            self.install(directory, &mut state, &work, &pending.invocation)?;
        }
        Ok(state)
    }
    /// The lease prevents an execution racing with publication. The caller
    /// receives a sealed retained revision, never a live container mount.
    pub fn snapshot_workspace(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        revision: u64,
        operation: &str,
    ) -> Result<Workspace> {
        let directory = self.directory(id)?;
        let _lock = self.lock(&directory)?;
        let state = self.reconcile(&directory, owner, session)?;
        if state.pending.is_some() {
            return Err(SandboxError::WorkspaceBusy);
        }
        if state.snapshot.revision != revision {
            return Err(SandboxError::StaleRevision);
        }
        let job_id = filesystem::digest(operation);
        let target = self.root.join("workspaces").join(&job_id);
        if target.exists() {
            return Err(SandboxError::OutcomeUnknown);
        }
        filesystem::private(&target)?;
        files::copy(&revision_path(&directory, revision), &target)?;
        Ok(execution_workspace(self, &job_id))
    }
}
impl TaskExecution {
    pub fn finish(self, receipt: &Receipt) -> Result<TaskSnapshot> {
        if receipt.invocation_id != self.operation || !receipt.cleanup_confirmed {
            return Err(SandboxError::OutcomeUnknown);
        }
        Ok(self
            .store
            .reconcile(&self.directory, &self.owner, &self.session)?
            .snapshot)
    }
    /// Only a definite pre-submit validation failure may release pending intent.
    pub fn reject(self) -> Result<()> {
        let mut state = self
            .store
            .load(&self.directory, &self.owner, &self.session)?;
        if state.pending.as_ref().map(|p| p.invocation.as_str()) != Some(self.operation.as_str()) {
            return Err(SandboxError::OutcomeUnknown);
        }
        state.pending = None;
        state.snapshot.running_invocation = None;
        self.store.save(&self.directory, &state)
    }
}
