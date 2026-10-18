// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Catalog, Result, ToolError, ToolRuntime, background, public_name};
use exoharness_contracts::invocation::InvocationState;
use exoharness_mcp::{Connection, ConnectionOptions};
use exoharness_storage_postgres::{PendingMcpTask, RegistryAccess};
use serde_json::json;
use std::time::Duration;

impl ToolRuntime {
    /// One bounded query of the original service-declared poll action. It never
    /// submits/cancels generation, consumes model tokens, or enters admission.
    pub async fn reconcile_mcp_task(
        &self,
        catalog: &Catalog,
        saved: &PendingMcpTask,
    ) -> Result<bool> {
        let task =
            background::declared(&json!({"_meta":{"exoharness":{"background_task":saved.task}}}))?
                .ok_or(ToolError::Unknown)?;
        let record = self
            .current(catalog, &saved.owner, &saved.capability)
            .await?;
        if record.revision as u64 != saved.revision
            || !saved.snapshot.capabilities.iter().any(|cap| {
                cap.id == saved.capability && cap.allowed && cap.revision == saved.revision
            })
        {
            return Err(ToolError::Authorization);
        }
        // Host recovery has no model activation set. Use only the authorized
        // catalog binding already frozen for this declared status action.
        let binding = catalog
            .bindings
            .get(&public_name(&saved.capability, &task.poll.tool))
            .ok_or(ToolError::Authorization)?;
        if !saved.snapshot.capabilities.iter().any(|cap| {
            cap.id == saved.capability
                && cap.tool_schemas.get(&binding.tool.name) == Some(&binding.tool.input_schema)
        }) {
            return Err(ToolError::Authorization);
        }
        let (registry, access, id) = (
            self.registry.clone(),
            RegistryAccess::User(saved.owner.clone()),
            saved.capability.clone(),
        );
        let permissions =
            tokio::task::spawn_blocking(move || registry.0.capability_permissions(&access, &id))
                .await
                .map_err(|_| ToolError::Storage)?
                .map_err(|_| ToolError::Authorization)?;
        let connection = Connection::connect(
            &record,
            &permissions,
            &self.mcp.secrets,
            ConnectionOptions::default(),
        )
        .await
        .map_err(|_| ToolError::Unknown)?;
        let result = async {
            let tools = connection.tools().await.map_err(|_| ToolError::Unknown)?;
            let live_schema = background::schema(&tools, &task.poll)?;
            if live_schema != &binding.tool.input_schema {
                return Err(ToolError::Authorization);
            }
            // Recheck immediately before sending the saved, job-specific poll.
            self.current(catalog, &saved.owner, &saved.capability)
                .await?;
            let value = connection
                .call(
                    &task.poll.tool,
                    task.poll.arguments.clone(),
                    |_| {},
                    std::future::pending::<()>(),
                )
                .await
                .map_err(|_| ToolError::Unknown)?;
            let observed = background::declared(&value)?.ok_or(ToolError::Unknown)?;
            if observed.id != task.id || observed.poll != task.poll {
                return Err(ToolError::Unknown);
            }
            let Some(receipt) = background::confirmed_terminal(value, &task.id) else {
                return Ok(false);
            };
            let next = if observed.status == "completed" && receipt["isError"] != true {
                InvocationState::Succeeded
            } else {
                InvocationState::FailedAfterExecution
            };
            let store = self.execution.clone();
            let saved = saved.clone();
            tokio::task::spawn_blocking(move || {
                store.resolve_invocation(
                    &saved.owner,
                    &saved.session,
                    &saved.run,
                    &saved.invocation,
                    next,
                    Some(receipt),
                )
            })
            .await
            .map_err(|_| ToolError::Storage)?
            .map_err(|_| ToolError::Storage)?;
            Ok(true)
        }
        .await;
        let _ = tokio::time::timeout(Duration::from_secs(2), connection.close()).await;
        result
    }
}
