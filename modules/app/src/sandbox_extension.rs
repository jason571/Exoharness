use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    resources::OwnedResource,
    services::{ExtensionContext, Service},
};
use exoharness_sandbox::Sandbox;
use std::{path::PathBuf, sync::Arc};
pub struct SandboxExtension {
    pub root: PathBuf,
    pub image: String,
}
struct Supervisor(Sandbox);
#[async_trait]
impl OwnedResource for Supervisor {
    async fn release(&mut self) -> Result<(), String> {
        self.0.stop().await.map_err(|e| e.to_string())
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        self.0.cancel_all();
    }
}
#[async_trait]
impl Extension for SandboxExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "sandbox".into(),
            provides: vec![Sandbox::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let backend = Sandbox::open(self.root.clone(), &self.image)
            .await
            .map_err(|e| e.to_string())?;
        context.own("OCI job supervisor", Box::new(Supervisor(backend.clone())))?;
        context
            .provide(Arc::new(backend))
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
