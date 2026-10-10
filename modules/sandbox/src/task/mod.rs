//! Scoped, revisioned task files. Containers only receive execution copies.
//! Metadata and retained revisions are never mounted into an execution.
mod coding;
mod edit;
mod execution;
mod files;
mod imports;
mod store;
use crate::{Sandbox, Workspace};
pub use execution::TaskExecution;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};
pub use store::TaskStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskFile {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub workspace_id: String,
    pub revision: u64,
    pub files: Vec<TaskFile>,
    pub skill_id: Option<String>,
    pub skill_revision: Option<i64>,
    pub notes: String,
    pub running_invocation: Option<String>,
    #[serde(default)]
    pub last_job: Option<TaskJob>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskJob {
    pub invocation_id: String,
    pub status: String,
    pub cleanup_confirmed: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskPatch {
    pub path: String,
    pub sha256: String,
    pub before: String,
    pub after: String,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskEdit {
    #[serde(default)]
    pub files: BTreeMap<String, String>,
    #[serde(default)]
    pub patches: Vec<TaskPatch>,
    #[serde(default)]
    pub deletes: Vec<String>,
    pub notes: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct State {
    owner: String,
    session: String,
    snapshot: TaskSnapshot,
    pending: Option<Pending>,
    operations: BTreeMap<String, u64>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Pending {
    invocation: String,
    workspace: String,
}
impl Sandbox {
    pub fn task_store(&self) -> crate::Result<TaskStore> {
        TaskStore::open(self.inner.root.clone())
    }
}
fn execution_workspace(store: &TaskStore, id: &str) -> Workspace {
    Workspace {
        root: store.root.join("workspaces").join(id),
        store: store.root.clone(),
        id: id.into(),
    }
}
fn revision_path(directory: &std::path::Path, revision: u64) -> PathBuf {
    directory.join(format!("revision-{revision}"))
}
