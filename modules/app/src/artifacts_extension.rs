use async_trait::async_trait;
use exoharness_api::ExecutionService;
use exoharness_artifacts::Artifacts;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use std::{path::PathBuf, sync::Arc};
pub struct ArtifactsExtension {
    pub root: PathBuf,
    pub public_base: String,
}
#[async_trait]
impl Extension for ArtifactsExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "artifacts".into(),
            provides: vec![Artifacts::KEY.into()],
            requires: vec![ExecutionService::KEY.into()],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let store = context
            .require::<ExecutionService>()
            .map_err(|e| e.to_string())?
            .0
            .clone();
        let root = self.root.clone();
        let public_base = self.public_base.clone();
        let artifacts =
            tokio::task::spawn_blocking(move || Artifacts::open(root, store, &public_base))
                .await
                .map_err(|_| "Artifact worker failed")?
                .map_err(|e| e.to_string())?;
        context
            .provide(Arc::new(artifacts))
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
