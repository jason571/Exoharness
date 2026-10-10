// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Registered MCP configuration is independent from transport generations.
use crate::{ImportRequest, McpError, Result, parse_import};
use exoharness_contracts::capability::{CapabilityScope, StoredCapability, ValidatedCapability};
use exoharness_core::services::Service;
use exoharness_registry::Registry;
use exoharness_secrets::SecretStore;
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use std::sync::{Arc, Mutex};
pub struct Mcp {
    pub registry: Arc<Registry>,
    pub secrets: Arc<SecretStore>,
    pub(crate) changes: Mutex<()>,
    validations: tokio::sync::Semaphore,
}
impl Service for Mcp {
    const KEY: &'static str = "mcp";
}
impl Mcp {
    pub fn new(registry: Arc<Registry>, secrets: Arc<SecretStore>) -> Self {
        Self {
            registry,
            secrets,
            changes: Mutex::new(()),
            validations: tokio::sync::Semaphore::new(4),
        }
    }
    pub fn import(
        &self,
        access: &RegistryAccess,
        request: &ImportRequest,
        actor: &str,
    ) -> Result<Vec<StoredCapability>> {
        let _guard = self.changes.lock().map_err(|_| McpError::Configuration)?;
        match (access, &request.scope) {
            (RegistryAccess::Administrator, _) => {}
            (RegistryAccess::User(id), CapabilityScope::User { user_id }) if id == user_id => {}
            _ => return Err(StoreError::NotAuthorized.into()),
        }
        let mut parsed = parse_import(request)?;
        let mut installed = Vec::new();
        if matches!(access, RegistryAccess::User(_)) && parsed.administrator_manifest {
            return Err(StoreError::NotAuthorized.into());
        }
        // Write new opaque secrets before DB commit. Nothing connects or executes.
        let mut database_attempted = false;
        let result = (|| {
            for credential in parsed.secrets {
                let key = self
                    .secrets
                    .put(&credential.value)
                    .map_err(|_| McpError::Secret)?;
                installed.push(key.clone());
                let reference = parsed.manifests[credential.index]
                    .secret_refs
                    .iter_mut()
                    .find(|r| r.name == credential.name)
                    .ok_or(McpError::Configuration)?;
                reference.key = key;
            }
            let checked = parsed
                .manifests
                .into_iter()
                .map(|manifest| {
                    ValidatedCapability::new(manifest, None).map_err(|_| McpError::Configuration)
                })
                .collect::<Result<Vec<_>>>()?;
            database_attempted = true;
            Ok(self
                .registry
                .0
                .register_capabilities(access, checked, actor, false)?)
        })();
        // A connection error may mean COMMIT succeeded with a lost receipt.
        // Never delete newly-written secrets after an ambiguous DB error.
        if !database_attempted
            || matches!(
                &result,
                Err(McpError::Store(
                    StoreError::Conflict | StoreError::Invalid(_) | StoreError::NotAuthorized
                ))
            )
        {
            for key in installed {
                let _ = self.secrets.delete(&key);
            }
        }
        result
    }
    pub async fn validate(
        &self,
        access: RegistryAccess,
        id: String,
        actor: String,
    ) -> Result<serde_json::Value> {
        let _permit = self
            .validations
            .acquire()
            .await
            .map_err(|_| McpError::Configuration)?;
        let registry = self.registry.clone();
        let copy_id = id.clone();
        let copy_access = access.clone();
        let (mut record, permissions) = tokio::task::spawn_blocking(move || {
            Ok::<_, StoreError>((
                registry.0.capability(&copy_access, &copy_id)?,
                registry.0.capability_permissions(&copy_access, &copy_id)?,
            ))
        })
        .await
        .map_err(|_| McpError::Configuration)??;
        // This explicit validation operation grants no execution/enabled state.
        record.enabled = true;
        let connected = crate::Connection::connect(
            &record,
            &permissions,
            &self.secrets,
            crate::ConnectionOptions::default(),
        )
        .await;
        let (result, tools, info) = match connected {
            Ok(connection) => {
                let tools = connection.tools().await;
                let info = connection.server_info();
                let closed = connection.close().await;
                match (tools, closed) {
                    (Ok(tools), Ok(())) => (Ok(()), Some(tools), info),
                    (Err(error), _) | (_, Err(error)) => {
                        (Err(error), None, serde_json::Value::Null)
                    }
                }
            }
            Err(error) => (Err(error), None, serde_json::Value::Null),
        };
        if matches!(
            &result,
            Err(McpError::Permission
                | McpError::SandboxRequired
                | McpError::Secret
                | McpError::Configuration)
        ) {
            return result.map(|_| serde_json::Value::Null);
        }
        let registry = self.registry.clone();
        let stored_tools = tools.clone();
        let revision = record.revision;
        tokio::task::spawn_blocking(move || {
            registry.0.observe_capability(
                &access,
                &id,
                revision,
                stored_tools.as_deref(),
                &info,
                &actor,
            )
        })
        .await
        .map_err(|_| McpError::Configuration)??;
        Ok(
            serde_json::json!({"status":if result.is_ok(){"passed"}else{"failed"},"message":if result.is_ok(){"标准MCP连接及工具发现通过"}else{"MCP连接或工具发现未完成；未调用业务工具"},"tools":tools.unwrap_or_default(),"revision":revision,"resumable":false}),
        )
    }
}
