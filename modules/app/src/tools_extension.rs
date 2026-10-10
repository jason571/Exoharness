//! One Core-owned provider bridge. Chat depends on it, so chat workers drain
//! before protocol providers, sandbox and artifact services are released.
use async_trait::async_trait;
use exoharness_api::ExecutionService;
use exoharness_artifacts::Artifacts;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_mcp::Mcp;
use exoharness_model_runtime::ModelRuntime;
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_skills::Skills;
use exoharness_tools::ToolRuntime;
use std::sync::Arc;
pub struct ToolsExtension {
    pub sandbox_enabled: bool,
}
#[async_trait]
impl Extension for ToolsExtension {
    fn spec(&self) -> ExtensionSpec {
        let mut requires = vec![
            ExecutionService::KEY.into(),
            Registry::KEY.into(),
            Skills::KEY.into(),
            Mcp::KEY.into(),
            Artifacts::KEY.into(),
            ModelRuntime::KEY.into(),
        ];
        if self.sandbox_enabled {
            requires.push(Sandbox::KEY.into());
        }
        ExtensionSpec {
            id: "tool-runtime".into(),
            provides: vec![ToolRuntime::KEY.into()],
            requires,
        }
    }
    async fn start(&mut self, ctx: &mut ExtensionContext<'_>) -> Result<(), String> {
        let service = ToolRuntime {
            execution: ctx
                .require::<ExecutionService>()
                .map_err(|e| e.to_string())?
                .0
                .clone(),
            registry: ctx.require::<Registry>().map_err(|e| e.to_string())?,
            skills: ctx.require::<Skills>().map_err(|e| e.to_string())?,
            mcp: ctx.require::<Mcp>().map_err(|e| e.to_string())?,
            artifacts: ctx.require::<Artifacts>().map_err(|e| e.to_string())?,
            model_runtime: Some(ctx.require::<ModelRuntime>().map_err(|e| e.to_string())?),
            sandbox: if self.sandbox_enabled {
                Some(ctx.require::<Sandbox>().map_err(|e| e.to_string())?)
            } else {
                None
            },
        };
        ctx.provide(Arc::new(service)).map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
