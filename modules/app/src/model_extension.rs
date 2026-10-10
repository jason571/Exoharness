//! Runtime service depends on configuration through the same typed Core graph.
use async_trait::async_trait;
use exoharness_configuration::Models;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_model_runtime::{ModelRuntime, RuntimePolicy};
use std::sync::Arc;
#[derive(Default)]
pub struct ModelExtension {
    runtime: Option<Arc<ModelRuntime>>,
}
#[async_trait]
impl Extension for ModelExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "model-runtime".into(),
            provides: vec![ModelRuntime::KEY.into()],
            requires: vec![Models::KEY.into()],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let models = context.require::<Models>().map_err(|e| e.to_string())?;
        let mut policy = RuntimePolicy::default();
        if let Ok(seconds) = std::env::var("EXOHARNESS_MODEL_REQUEST_TIMEOUT_SECONDS") {
            policy.timeout = std::time::Duration::from_secs(
                seconds.parse().map_err(|_| "invalid model timeout")?,
            );
        }
        let runtime = Arc::new(ModelRuntime::new(models, policy).map_err(|e| e.to_string())?);
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
