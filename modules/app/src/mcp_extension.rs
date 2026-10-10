//! Standard extensions declare service dependencies; no connection at boot.
use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_mcp::Mcp;
use exoharness_registry::Registry;
use exoharness_secrets::SecretStore;
use std::{path::PathBuf, sync::Arc};
pub struct SecretsExtension {
    pub root: PathBuf,
}
#[async_trait]
impl Extension for SecretsExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "secrets".into(),
            provides: vec![SecretStore::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let root = self.root.clone();
        let secrets = tokio::task::spawn_blocking(move || SecretStore::open(root))
            .await
            .map_err(|_| "Secret storage worker failed")?
            .map_err(|_| "Private Secret Store is unavailable")?;
        context
            .provide(Arc::new(secrets))
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
pub struct McpExtension;
#[async_trait]
impl Extension for McpExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "mcp".into(),
            provides: vec![Mcp::KEY.into()],
            requires: vec![Registry::KEY.into(), SecretStore::KEY.into()],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let registry = context.require::<Registry>().map_err(|e| e.to_string())?;
        let secrets = context
            .require::<SecretStore>()
            .map_err(|e| e.to_string())?;
        context
            .provide(Arc::new(Mcp::new(registry, secrets)))
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
