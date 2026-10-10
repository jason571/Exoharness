// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "role", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    System {
        text: String,
    },
    User {
        text: String,
    },
    /// Read-only runtime context at its original journal position. Not a user
    /// turn, an assistant answer, or an executable tool proposal.
    RuntimeData {
        text: String,
    },
    Assistant {
        text: String,
        calls: Vec<ToolCall>,
    },
    ToolResult {
        call_id: String,
        content: String,
        is_error: bool,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum ToolChoice {
    #[default]
    Auto,
    None,
    Required,
    Named(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<Tool>,
    pub tool_choice: ToolChoice,
    pub output_token_limit: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub cache_read_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Finish {
    Stop,
    ToolCalls,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Completion {
    pub response_id: Option<String>,
    pub model_name: String,
    pub provider_name: String,
    pub model_id: String,
    pub configuration_revision: u64,
    pub text: String,
    pub calls: Vec<ToolCall>,
    pub finish: Finish,
    pub usage: Usage,
}
