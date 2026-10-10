// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Authenticated loopback infrastructure validation API, not a chat backend.
mod application;
mod chat;
pub use chat::{
    ChatTasks, TaskAdmissionError, chat_router, chat_router_with_tasks, chat_router_with_tools,
};
pub mod auth;
mod browser_sessions;
mod configuration;
mod console;
mod conversations;
mod errors;
mod events;
mod identity;
mod knowledge;
mod management;
mod mcp;
mod media;
mod registry;
mod routes;
mod runtime;
mod trace;
pub use media::{artifact_management_router, media_router};
mod skills;
mod state;

pub use configuration::{
    configuration_router, configuration_router_with_registry, configuration_router_with_runtime,
};
pub use conversations::conversation_router;
pub use identity::{identity_router, identity_router_with_registry};
pub use knowledge::{knowledge_router, knowledge_router_with_sandbox};
pub use management::management_router;
pub use mcp::mcp_router;
pub use registry::{registry_router, registry_router_with_runtime};
pub use routes::router;
pub use runtime::{runtime_router, runtime_router_with_tools};
pub use skills::skills_router;
pub use state::{ApiState, ExecutionService, Surface};
