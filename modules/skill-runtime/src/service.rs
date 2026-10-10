use crate::{
    EnvironmentEvidence, Result, RuntimeError, RuntimeStatus, cache::Cache, metadata, probe,
    workers::Workers,
};
use exoharness_contracts::capability::PermissionDecision;
use exoharness_core::services::Service;
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_skills::{PackageSnapshot, Skills};
use exoharness_storage_postgres::RegistryAccess;
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

pub struct SkillRuntime {
    skills: Arc<Skills>,
    registry: Arc<Registry>,
    sandbox: Arc<Sandbox>,
    cache: Arc<Cache>,
    checks: Semaphore,
    workers: Workers,
}
impl Service for SkillRuntime {
    const KEY: &'static str = "skill-runtime";
}
impl SkillRuntime {
    pub fn open(
        root: PathBuf,
        skills: Arc<Skills>,
        registry: Arc<Registry>,
        sandbox: Arc<Sandbox>,
    ) -> Result<Self> {
        Ok(Self {
            skills,
            registry,
            sandbox,
            cache: Arc::new(Cache::open(root)?),
            checks: Semaphore::new(4),
            workers: Workers::new(),
        })
    }
    async fn snapshot(
        &self,
        access: RegistryAccess,
        id: String,
        revision: i64,
    ) -> Result<PackageSnapshot> {
        let skills = self.skills.clone();
        self.workers
            .run(move || {
                skills
                    .package_snapshot(&access, &id, revision)
                    .map_err(Into::into)
            })
            .await
    }
    async fn permitted(&self, access: RegistryAccess, id: String, revision: i64) -> Result<()> {
        let registry = self.registry.clone();
        self.workers
            .run(move || {
                let record = registry.0.capability(&access, &id)?;
                if record.revision != revision {
                    return Err(RuntimeError::Conflict);
                }
                let permissions = registry.0.capability_permissions(&access, &id)?;
                if !record.enabled
                    || permissions
                        .iter()
                        .any(|p| p.decision != PermissionDecision::Allow)
                    || record.manifest.permissions.len() != permissions.len()
                {
                    return Err(RuntimeError::Permission);
                }
                Ok(())
            })
            .await
    }
    pub async fn status(
        &self,
        access: RegistryAccess,
        id: String,
        revision: i64,
    ) -> Result<RuntimeStatus> {
        let package = self.snapshot(access.clone(), id.clone(), revision).await?;
        let metadata = metadata::extract(&package)?;
        let scripts_present = !metadata.runtimes.is_empty();
        let digest = package
            .capability
            .source_digest
            .ok_or(RuntimeError::Declaration)?;
        let image = self.sandbox.image_id().to_owned();
        let cache = self.cache.clone();
        let evidence = self
            .workers
            .run(move || cache.read(&id, revision, &digest, &image))
            .await?;
        let environment_ready = evidence.as_ref().is_some_and(|e| e.ready);
        // Preparing dependencies never silently enables a Skill or grants an
        // executable Tool. The eventual dispatcher must separately authorize.
        Ok(RuntimeStatus {
            scripts_present,
            environment_ready,
            script_execution_ready: false,
            evidence,
        })
    }
    pub async fn prepare(
        &self,
        access: RegistryAccess,
        id: String,
        revision: i64,
    ) -> Result<RuntimeStatus> {
        if !matches!(access, RegistryAccess::Administrator) {
            return Err(RuntimeError::Permission);
        }
        // Bounded admission; do not spawn unbounded blocking workers/containers.
        let _permit = self
            .checks
            .try_acquire()
            .map_err(|_| RuntimeError::Unavailable)?;
        let mut stopped = self.workers.stopped.subscribe();
        if *stopped.borrow() {
            return Err(RuntimeError::Unavailable);
        }
        tokio::select! {biased;_=stopped.changed()=>Err(RuntimeError::Unavailable),result=self.prepare_once(access,id,revision)=>result}
    }
    pub async fn close(&self) {
        self.workers.stopped.send_replace(true);
        let _checks = self.checks.acquire_many(4).await;
        self.workers.drain().await;
    }
    async fn prepare_once(
        &self,
        access: RegistryAccess,
        id: String,
        revision: i64,
    ) -> Result<RuntimeStatus> {
        let package = self.snapshot(access.clone(), id.clone(), revision).await?;
        let metadata = metadata::extract(&package)?;
        if metadata.runtimes.is_empty() {
            return Ok(RuntimeStatus {
                scripts_present: false,
                environment_ready: false,
                script_execution_ready: false,
                evidence: None,
            });
        }
        self.permitted(access.clone(), id.clone(), revision).await?;
        let checked = probe::check(&self.sandbox, &metadata).await?;
        // If update/delete/revocation happened during the check, do not publish
        // evidence as a proof for the new revision or a newly denied capability.
        self.permitted(access.clone(), id.clone(), revision).await?;
        let source_digest = package
            .capability
            .source_digest
            .ok_or(RuntimeError::Declaration)?;
        let evidence = EnvironmentEvidence {
            capability_id: id,
            revision,
            source_digest,
            image_id: self.sandbox.image_id().into(),
            ready: checked.ready,
            runtimes: metadata.runtimes.into_iter().map(str::to_owned).collect(),
            missing: checked.missing,
            versions: checked.versions,
        };
        let cache = self.cache.clone();
        let saved = evidence.clone();
        self.workers.run(move || cache.write(&saved)).await?;
        Ok(RuntimeStatus {
            scripts_present: true,
            environment_ready: evidence.ready,
            script_execution_ready: false,
            evidence: Some(evidence),
        })
    }
}
