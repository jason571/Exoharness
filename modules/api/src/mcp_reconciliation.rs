// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Lifecycle-owned recovery for late MCP receipts; no model or generator retry.
use exoharness_configuration::Assistants;
use exoharness_tools::{ToolError, ToolRuntime};
use std::{sync::Arc, time::Duration};
use tokio::sync::watch;

/// Poll one bounded page. The returned cursor provides fairness across jobs.
pub async fn reconcile_mcp_page(
    tools: Arc<ToolRuntime>,
    assistants: Arc<Assistants>,
    after: String,
) -> Result<String, exoharness_storage_postgres::StoreError> {
    let store = tools.execution.clone();
    let tasks = tokio::task::spawn_blocking(move || store.pending_mcp_tasks(&after, 16))
        .await
        .map_err(|_| exoharness_storage_postgres::StoreError::Database)??;
    let cursor = if tasks.len() == 16 {
        tasks.last().unwrap().invocation.clone()
    } else {
        String::new()
    };
    for task in tasks {
        let (tools_copy, assistants_copy, saved) =
            (tools.clone(), assistants.clone(), task.clone());
        let catalog = tokio::task::spawn_blocking(move || {
            assistants_copy.refresh().map_err(|_| ToolError::Storage)?;
            let profile = assistants_copy
                .detail(&saved.snapshot.assistant_id)
                .filter(|profile| {
                    profile["enabled"] == true
                        && profile["active_revision"].as_u64()
                            == Some(saved.snapshot.assistant_revision)
                })
                .ok_or(ToolError::Authorization)?;
            tools_copy.catalog(&saved.owner, profile)
        })
        .await;
        let result = match catalog {
            Ok(Ok(catalog)) => tokio::time::timeout(
                Duration::from_secs(8),
                tools.reconcile_mcp_task(&catalog, &task),
            )
            .await
            .unwrap_or(Err(ToolError::Unknown)),
            Ok(Err(error)) => Err(error),
            Err(_) => Err(ToolError::Storage),
        };
        let state = match result {
            Ok(true) => "verified",
            Ok(false) => "pending",
            Err(ToolError::Authorization) => "blocked",
            Err(_) => "unavailable",
        };
        let store = tools.execution.clone();
        tokio::task::spawn_blocking(move || store.note_mcp_reconciliation(&task, state))
            .await
            .map_err(|_| exoharness_storage_postgres::StoreError::Database)??;
    }
    Ok(cursor)
}

/// Caller retains and joins this future's task before releasing MCP/database services.
pub async fn reconcile_mcp_tasks(
    tools: Arc<ToolRuntime>,
    assistants: Arc<Assistants>,
    mut stop: watch::Receiver<bool>,
) {
    let mut cursor = String::new();
    loop {
        if *stop.borrow() {
            break;
        }
        tokio::select! {biased;
            _=stop.changed()=>break,
            result=reconcile_mcp_page(tools.clone(),assistants.clone(),cursor.clone())=>{
                match result {Ok(next)=>cursor=next, Err(_)=>eprintln!("MCP background reconciliation storage unavailable")}
            }
        }
        tokio::select! {biased; _=stop.changed()=>break, ()=tokio::time::sleep(Duration::from_secs(15))=>{}}
    }
}
