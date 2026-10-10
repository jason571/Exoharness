// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Chat workers are one Core-owned service, shared by both HTTP surfaces.
use async_trait::async_trait;
use exoharness_api::{ChatTasks, ExecutionService};
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_model_runtime::ModelRuntime;
use std::sync::Arc;
#[derive(Default)]
pub struct ChatExtension {
    tasks: Option<Arc<ChatTasks>>,
    execution: Option<Arc<ExecutionService>>,
    pending: Vec<String>,
}
#[async_trait]
impl Extension for ChatExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "chat-workers".into(),
            provides: vec![ChatTasks::KEY.into()],
            requires: vec![
                ModelRuntime::KEY.into(),
                ExecutionService::KEY.into(),
                exoharness_tools::ToolRuntime::KEY.into(),
            ],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        context
            .require::<exoharness_tools::ToolRuntime>()
            .map_err(|e| e.to_string())?;
        context
            .require::<ModelRuntime>()
            .map_err(|e| e.to_string())?;
        self.execution = Some(
            context
                .require::<ExecutionService>()
                .map_err(|e| e.to_string())?,
        );
        let tasks = Arc::new(ChatTasks::default());
        context.provide(tasks.clone()).map_err(|e| e.to_string())?;
        self.tasks = Some(tasks);
        Ok(())
    }
    async fn stop(&mut self) -> Result<(), String> {
        if let Some(tasks) = &self.tasks {
            self.pending.extend(tasks.close().await);
            if let Some(execution) = &self.execution {
                let store = execution.0.clone();
                let ids = self.pending.clone();
                tokio::task::spawn_blocking(move || store.finish_stopped_chats(&ids))
                    .await
                    .map_err(|_| "chat cleanup worker failed")?
                    .map_err(|e| e.to_string())?;
            }
        }
        self.pending.clear();
        self.tasks.take();
        self.execution.take();
        Ok(())
    }
}
