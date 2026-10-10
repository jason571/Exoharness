use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_skill_runtime::SkillRuntime;
use exoharness_skills::Skills;
use std::{path::PathBuf, sync::Arc};

pub struct SkillRuntimeExtension {
    pub root: PathBuf,
    pub runtime: Option<Arc<SkillRuntime>>,
}
#[async_trait]
impl Extension for SkillRuntimeExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "skill-runtime".into(),
            provides: vec![SkillRuntime::KEY.into()],
            requires: vec![
                Skills::KEY.into(),
                Registry::KEY.into(),
                Sandbox::KEY.into(),
            ],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let skills = context.require::<Skills>().map_err(|e| e.to_string())?;
        let registry = context.require::<Registry>().map_err(|e| e.to_string())?;
        let sandbox = context.require::<Sandbox>().map_err(|e| e.to_string())?;
        let root = self.root.clone();
        let runtime = tokio::task::spawn_blocking(move || {
            SkillRuntime::open(root, skills, registry, sandbox)
        })
        .await
        .map_err(|_| "Skill runtime worker failed")?
        .map_err(|e| e.to_string())?;
        let runtime = Arc::new(runtime);
        context
            .provide(runtime.clone())
            .map_err(|e| e.to_string())?;
        self.runtime = Some(runtime);
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), String> {
        if let Some(runtime) = self.runtime.take() {
            runtime.close().await;
        }
        Ok(())
    }
}
