// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Original chat API compatibility; execution, persistence and stream are split.
mod attachments;
mod control;
mod publication_checkpoint;
mod run_admission;
mod start;
mod stream;
mod tasks;
mod text_driver;
mod tool_driver;
mod worker;
use crate::{ApiState, auth};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use exoharness_configuration::{Assistants, Models};
use exoharness_model_runtime::ModelRuntime;
use exoharness_storage_postgres::PostgresStore;
use std::sync::Arc;
pub use tasks::{ChatTasks, TaskAdmissionError};

#[derive(Clone)]
struct ChatState {
    store: PostgresStore,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    runtime: Arc<ModelRuntime>,
    stop: tokio::sync::watch::Receiver<bool>,
    tasks: Arc<ChatTasks>,
    tools: Option<Arc<exoharness_tools::ToolRuntime>>,
}
pub fn chat_router(
    api: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    runtime: Arc<ModelRuntime>,
) -> Router {
    chat_router_with_tasks(
        api,
        models,
        assistants,
        runtime,
        Arc::new(ChatTasks::default()),
    )
}
pub fn chat_router_with_tasks(
    api: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    runtime: Arc<ModelRuntime>,
    tasks: Arc<ChatTasks>,
) -> Router {
    chat_router_with_tools(api, models, assistants, runtime, tasks, None)
}
pub fn chat_router_with_tools(
    api: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    runtime: Arc<ModelRuntime>,
    tasks: Arc<ChatTasks>,
    tools: Option<Arc<exoharness_tools::ToolRuntime>>,
) -> Router {
    let state = ChatState {
        store: api.execution.0.clone(),
        models,
        assistants,
        runtime,
        stop: api.stop.clone(),
        tasks,
        tools,
    };
    Router::new()
        .route(
            "/v1/sessions/{session}/turns",
            post(start::turn).layer(DefaultBodyLimit::max(attachments::MAX_REQUEST_BYTES)),
        )
        .route("/v1/runs/{run}/events", get(stream::events))
        .route("/v1/runs/{run}/cancel", post(control::cancel))
        .route("/v1/runs/{run}/resume", post(control::resume))
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .with_state(state)
}
