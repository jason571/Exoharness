// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    Result, RunSpec, SandboxError, Workspace,
    engine::Engine,
    filesystem,
    job::{JobHandle, OwnedJob},
};
use exoharness_core::services::Service;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Mutex},
};
pub(crate) struct Inner {
    pub root: PathBuf,
    pub engine: Engine,
    pub tasks: Mutex<BTreeMap<String, OwnedJob>>,
    pub pending: Mutex<BTreeSet<String>>,
    pub closing: Mutex<bool>,
    pub workspaces: Mutex<BTreeSet<String>>,
    pub lease: Mutex<Option<std::fs::File>>,
}
#[derive(Clone)]
pub struct Sandbox {
    pub(crate) inner: Arc<Inner>,
}
impl Service for Sandbox {
    const KEY: &'static str = "sandbox";
}
impl Sandbox {
    pub async fn open(root: PathBuf, image: &str) -> Result<Self> {
        if !root.is_absolute()
            || root
                .to_string_lossy()
                .chars()
                .any(|c| c == ',' || c.is_control())
        {
            return Err(SandboxError::Invalid);
        }
        if !root.try_exists().map_err(|_| SandboxError::Storage)? {
            filesystem::private(root.parent().ok_or(SandboxError::Invalid)?)?;
        }
        filesystem::private(&root)?;
        filesystem::private(&root.join("jobs"))?;
        filesystem::private(&root.join("workspaces"))?;
        let lease = filesystem::lease(&root)?;
        let engine = Engine::open(&root, image).await?;
        engine.recover(&root).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                root,
                engine,
                tasks: Mutex::new(BTreeMap::new()),
                pending: Mutex::new(BTreeSet::new()),
                closing: Mutex::new(false),
                workspaces: Mutex::new(BTreeSet::new()),
                lease: Mutex::new(Some(lease)),
            }),
        })
    }
    pub fn image_id(&self) -> &str {
        &self.inner.engine.image
    }
    pub fn cancel_all(&self) {
        if let Ok(tasks) = self.inner.tasks.lock() {
            for job in tasks.values() {
                let _ = job.cancel.send(true);
            }
        }
    }
    pub fn cancel_invocation(&self, id: &str) -> Result<bool> {
        let tasks = self
            .inner
            .tasks
            .lock()
            .map_err(|_| SandboxError::Unavailable)?;
        if let Some(job) = tasks.get(id) {
            job.cancel
                .send(true)
                .map_err(|_| SandboxError::CleanupPending)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
    pub fn receipt(&self, id: &str) -> Result<Option<crate::Receipt>> {
        filesystem::receipt(&self.inner.root, id)
    }
    pub fn create_workspace(
        &self,
        id: &str,
        files: BTreeMap<String, Vec<u8>>,
    ) -> Result<Workspace> {
        if *self
            .inner
            .closing
            .lock()
            .map_err(|_| SandboxError::Unavailable)?
        {
            return Err(SandboxError::Unavailable);
        }
        filesystem::workspace(&self.inner.root, id, files)
    }
    pub fn start(&self, spec: RunSpec) -> Result<JobHandle> {
        let closing = self
            .inner
            .closing
            .lock()
            .map_err(|_| SandboxError::Unavailable)?;
        if *closing {
            return Err(SandboxError::Unavailable);
        }
        if !spec.limits.validate()
            || spec.arguments.len() > 128
            || spec
                .arguments
                .iter()
                .any(|v| v.len() > 8192 || v.contains('\0'))
        {
            return Err(SandboxError::Invalid);
        }
        if !spec.runtime.accepts(&spec.entrypoint) {
            return Err(SandboxError::RuntimeMismatch);
        }
        filesystem::verify(&self.inner.root, &spec.workspace, &spec.entrypoint)?;
        if let Some(package) = &spec.read_only_package {
            if package.store != self.inner.root
                || package.root != self.inner.root.join("workspaces").join(&package.id)
            {
                return Err(SandboxError::Invalid);
            }
            filesystem::private(&package.root)?;
        }
        let workspace_id = spec.workspace.id.clone();
        let mut active = self
            .inner
            .workspaces
            .lock()
            .map_err(|_| SandboxError::Unavailable)?;
        if !active.insert(workspace_id.clone()) {
            return Err(SandboxError::WorkspaceBusy);
        }
        let result = filesystem::reserve(&self.inner.root, &spec.invocation_id)
            .and_then(|_| crate::job::spawn(self.inner.clone(), spec));
        if result.is_err() {
            active.remove(&workspace_id);
        }
        result
    }
    pub async fn stop(&self) -> Result<()> {
        *self
            .inner
            .closing
            .lock()
            .map_err(|_| SandboxError::Unavailable)? = true;
        let tasks = self
            .inner
            .tasks
            .lock()
            .map_err(|_| SandboxError::Unavailable)?
            .values()
            .map(|job| (job.cancel.clone(), job.finished.clone()))
            .collect::<Vec<_>>();
        for (cancel, _) in &tasks {
            let _ = cancel.send(true);
        }
        // Await cloneable completion state, not take() ownership out of the
        // registry. Cancelling this stop future must not detach live resources.
        for (_, mut finished) in tasks {
            while !*finished.borrow() {
                finished
                    .changed()
                    .await
                    .map_err(|_| SandboxError::CleanupPending)?;
            }
        }
        let pending = self
            .inner
            .pending
            .lock()
            .map_err(|_| SandboxError::CleanupPending)?
            .clone();
        for name in pending {
            self.inner.engine.remove(&name).await?;
            self.inner
                .pending
                .lock()
                .map_err(|_| SandboxError::CleanupPending)?
                .remove(&name);
        }
        let mut lease = self
            .inner
            .lease
            .lock()
            .map_err(|_| SandboxError::CleanupPending)?;
        if let Some(file) = lease.as_ref() {
            // A concurrently forked CLI process may briefly inherit this open
            // file description before exec closes CLOEXEC descriptors. Closing
            // our fd alone is not sufficient to release its flock immediately.
            fs2::FileExt::unlock(file).map_err(|_| SandboxError::CleanupPending)?;
        }
        lease.take();
        Ok(())
    }
}
#[cfg(test)]
#[path = "../tests/unit/provider_lease.rs"]
mod lease_tests;
