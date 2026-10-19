// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Scoped tool providers: instructions, protocol tools and isolated execution.
//! Registry snapshots are private evidence; model schemas are loaded on demand.
mod admission;
mod background;
mod catalog;
mod catalog_search;
mod coding;
mod coding_schema;
mod discovery;
mod dispatch;
mod mcp_delivery;
mod model_tools;
mod names;
mod native;
mod publication;
mod reconciliation;
mod result_artifacts;
mod sandbox;
mod schema;
mod selection;
mod service;
mod skill_context;
mod skill_resources;
mod workflow;
mod workspace;
pub use admission::allowed;
pub use catalog::{Binding, Catalog};
pub use names::public_name;
pub use schema::{input_diagnostics, validate};
pub use service::{DispatchResult, ToolRuntime};
pub const CATALOG_ID: &str = "tool:native:catalog";
pub const SANDBOX_ID: &str = "tool:native:sandbox";

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("{0}")]
    UnavailableBeforeExecution(&'static str),
    #[error("{0}")]
    CorrectableInput(String),
    #[error("声明的输出文件不存在：{0}")]
    MissingOutput(String),
    #[error("能力登记或执行记录不可用")]
    Storage,
    #[error("能力未获授权、已停用或版本发生变化；未执行")]
    Authorization,
    #[error("{0}")]
    Input(String),
    #[error("结果尚未确认；不会重复提交")]
    Unknown,
}
pub type Result<T> = std::result::Result<T, ToolError>;
