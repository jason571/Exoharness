// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Catalog, Result, ToolError, admission};
use exoharness_artifacts::Artifacts;
use exoharness_contracts::capability::{CapabilitySpec, StoredCapability};
use exoharness_core::services::Service;
use exoharness_mcp::Mcp;
use exoharness_model_runtime::ModelRuntime;
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_skills::Skills;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::Value;
use std::sync::Arc;

pub struct ToolRuntime {
    pub execution: exoharness_storage_postgres::PostgresStore,
    pub registry: Arc<Registry>,
    pub skills: Arc<Skills>,
    pub mcp: Arc<Mcp>,
    pub sandbox: Option<Arc<Sandbox>>,
    pub artifacts: Arc<Artifacts>,
    pub model_runtime: Option<Arc<ModelRuntime>>,
}
impl Service for ToolRuntime {
    const KEY: &'static str = "tool-runtime";
}
pub struct DispatchResult {
    pub value: Value,
    pub executed: bool,
    pub failed: bool,
    pub artifacts: Vec<Value>,
}
impl ToolRuntime {
    /// Read the existing registered MCP limit after authorization/revision
    /// checks. Native, sandbox and model tools use their own execution paths.
    pub async fn mcp_concurrency(
        &self,
        catalog: &Catalog,
        owner: &str,
        capability: &str,
    ) -> Result<Option<u32>> {
        if !catalog.records.contains_key(capability) {
            return Ok(None);
        }
        let record = self.current(catalog, owner, capability).await?;
        Ok(match record.manifest.spec {
            CapabilitySpec::McpServer(spec) => Some(spec.limits.max_concurrency),
            _ => None,
        })
    }
    pub fn catalog(&self, owner: &str, profile: Value) -> Result<Catalog> {
        let mut catalog = Catalog::load(
            &self.registry,
            &RegistryAccess::User(owner.into()),
            profile,
            self.sandbox.is_some(),
        )?;
        if let Some(runtime) = &self.model_runtime {
            catalog.add_models(runtime.speech_models().map_err(|_| ToolError::Storage)?);
        }
        Ok(catalog)
    }
    pub(crate) async fn current(
        &self,
        catalog: &Catalog,
        owner: &str,
        id: &str,
    ) -> Result<StoredCapability> {
        let frozen = catalog
            .records
            .get(id)
            .ok_or(ToolError::Authorization)?
            .clone();
        let registry = self.registry.clone();
        let profile = catalog.profile.clone();
        let access = RegistryAccess::User(owner.into());
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let view = exoharness_contracts::capability::RegisteredCapabilityView {
                capability: registry
                    .0
                    .capability(&access, &id)
                    .map_err(|_| ToolError::Authorization)?,
                permissions: registry
                    .0
                    .capability_permissions(&access, &id)
                    .map_err(|_| ToolError::Authorization)?,
                health: None,
            };
            let kind = match view.capability.manifest.spec {
                CapabilitySpec::Skill(_) => "skills",
                CapabilitySpec::McpServer(_) => "mcp",
                CapabilitySpec::ToolNode(_) => "tools",
            };
            if !admission::granted(&view)
                || view.capability.revision != frozen.revision
                || view.capability.manifest_digest != frozen.manifest_digest
                || !admission::allowed(&profile, kind, &id)
            {
                return Err(ToolError::Authorization);
            }
            Ok(view.capability)
        })
        .await
        .map_err(|_| ToolError::Storage)?
    }
}
