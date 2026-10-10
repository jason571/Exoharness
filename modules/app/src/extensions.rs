use async_trait::async_trait;
use exoharness_api::ExecutionService;
use exoharness_configuration::{Assistants, Models};
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_identity::{AuthStore, mailer::MailService};
use exoharness_registry::Registry;
use std::sync::Arc;

pub struct DatabaseExtension(pub Arc<ExecutionService>);
pub struct IdentityExtension {
    pub store: Arc<AuthStore>,
    pub mailer: Arc<MailService>,
}
#[async_trait]
impl Extension for IdentityExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "identity".into(),
            provides: vec![AuthStore::KEY.into(), MailService::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        context
            .provide(self.store.clone())
            .map_err(|e| e.to_string())?;
        context
            .provide(self.mailer.clone())
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
#[async_trait]
impl Extension for DatabaseExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "postgres-execution".into(),
            provides: vec![ExecutionService::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        context.provide(self.0.clone()).map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct ConfigurationExtension {
    pub models: Arc<Models>,
    pub assistants: Arc<Assistants>,
}

pub struct RegistryExtension(pub Arc<Registry>);
#[async_trait]
impl Extension for RegistryExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "postgres-registry".into(),
            provides: vec![Registry::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        context.provide(self.0.clone()).map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
#[async_trait]
impl Extension for ConfigurationExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "deployment-configuration".into(),
            provides: vec![Models::KEY.into(), Assistants::KEY.into()],
            requires: vec![],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        context
            .provide(self.models.clone())
            .map_err(|e| e.to_string())?;
        context
            .provide(self.assistants.clone())
            .map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
