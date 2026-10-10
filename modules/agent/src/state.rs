// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_contracts::{
    model::{ToolCall, Usage},
    retry::ConsecutiveCalls,
};
use exoharness_core::context::JournalEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct EnginePolicy {
    pub max_model_calls: u64,
    pub max_tool_calls: u64,
    pub max_reported_tokens: u64,
    pub max_context_bytes: usize,
    pub output_token_limit: u64,
    pub requires_execution: bool,
}
impl EnginePolicy {
    pub(crate) fn valid(&self) -> bool {
        self.max_model_calls > 0
            && self.max_tool_calls > 0
            && self.max_reported_tokens > 0
            && self.max_context_bytes > 0
            && self.output_token_limit > 0
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCall {
    pub key: String,
    pub call: ToolCall,
    pub admitted: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason {
    Cancelled,
    ModelBudget,
    ToolBudget,
    TokenBudget,
    ContextBudget,
    RepeatedCall,
    MissingExecution,
    InvalidProposal,
    ModelIncomplete,
    InputRejected,
    ToolFailed,
    OutcomeUnknown,
    PersistenceFailed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineState {
    pub version: u32,
    pub entries: Vec<JournalEntry>,
    pub pending: Vec<PendingCall>,
    pub model_calls: u64,
    pub in_flight_model: Option<String>,
    pub tool_calls: u64,
    pub reported_tokens: u64,
    pub usage: Usage,
    pub unreported_requests: u64,
    pub successful_executions: u64,
    /// Run-wide correction budget; persisted and never reset by discovery or
    /// unchanged Resume. Two corrections allow at most three rejected inputs.
    #[serde(default)]
    pub input_corrections: u8,
    pub repeated: ConsecutiveCalls,
    pub blocked: Option<StopReason>,
    pub answer: Option<String>,
}
impl EngineState {
    pub fn new(entries: Vec<JournalEntry>, repeat_limit: usize) -> Result<Self, &'static str> {
        Ok(Self {
            version: 1,
            entries,
            pending: vec![],
            model_calls: 0,
            in_flight_model: None,
            tool_calls: 0,
            reported_tokens: 0,
            usage: Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            },
            unreported_requests: 0,
            successful_executions: 0,
            input_corrections: 0,
            repeated: ConsecutiveCalls::new(repeat_limit)?,
            blocked: None,
            answer: None,
        })
    }
    /// Account provider-reported usage, including a terminal error receipt.
    /// Unknown usage remains unknown; adapters must not estimate model tokens.
    pub fn add_usage(&mut self, usage: &Usage) {
        fn sum(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            a.zip(b).map(|(a, b)| a.saturating_add(b))
        }
        self.reported_tokens = self
            .reported_tokens
            .saturating_add(usage.input_tokens.unwrap_or(0))
            .saturating_add(usage.output_tokens.unwrap_or(0));
        if usage.input_tokens.is_none() || usage.output_tokens.is_none() {
            self.unreported_requests = self.unreported_requests.saturating_add(1);
        }
        self.usage.input_tokens = sum(self.usage.input_tokens, usage.input_tokens);
        self.usage.output_tokens = sum(self.usage.output_tokens, usage.output_tokens);
        self.usage.cache_read_tokens = sum(self.usage.cache_read_tokens, usage.cache_read_tokens);
        self.usage.cache_write_tokens =
            sum(self.usage.cache_write_tokens, usage.cache_write_tokens);
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum RunOutcome {
    Finished(String),
    Stopped(StopReason),
}
