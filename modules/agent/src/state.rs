// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_contracts::{
    model::{ToolCall, Usage},
    retry::ConsecutiveCalls,
};
use exoharness_core::context::JournalEntry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct EnginePolicy {
    pub max_model_calls: u64,
    pub max_tool_calls: u64,
    pub max_reported_tokens: u64,
    pub max_recovery_tokens: u64,
    pub recovery_output_cap_available: bool,
    pub max_context_bytes: usize,
    pub output_token_limit: u64,
    pub requires_execution: bool,
}
impl EnginePolicy {
    pub(crate) fn valid(&self) -> bool {
        self.max_model_calls > 0
            && self.max_tool_calls > 0
            && self.max_reported_tokens > 0
            && self.max_recovery_tokens > 0
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
    RecoveryTokenBudget,
    UnchangedCorrection,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxRecoveryTarget {
    pub tool: String,
    pub workspace: String,
    pub entrypoint: String,
    pub runtime: String,
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
    #[serde(default)]
    pub recovery_tokens: u64,
    #[serde(default)]
    pub recovery_token_limit: u64,
    #[serde(default)]
    pub recovery_active: bool,
    #[serde(default)]
    pub recovery_accounting_initialized: bool,
    pub usage: Usage,
    pub unreported_requests: u64,
    pub successful_executions: u64,
    /// Run-wide correction budget; persisted and never reset by discovery or
    /// unchanged Resume. Two corrections allow at most three rejected inputs.
    #[serde(default)]
    pub input_corrections: u8,
    /// Exact tools awaiting a valid replacement after before-execution input
    /// rejection. Older checkpoints default to no attributable episode.
    #[serde(default)]
    pub input_recovery_targets: BTreeSet<String>,
    #[serde(default)]
    pub input_recovery_started: bool,
    /// Bounded replans for host-confirmed unavailability before submission.
    #[serde(default)]
    pub service_recoveries: u8,
    /// Run-wide, persisted correction budget for terminated OCI code failures.
    #[serde(default)]
    pub sandbox_corrections: u8,
    #[serde(default)]
    pub sandbox_recovery_targets: Vec<SandboxRecoveryTarget>,
    #[serde(default)]
    pub sandbox_recovery_started: bool,
    /// One tool-free explanation after repeat protection refuses a proposal.
    /// Persisted so Resume cannot reopen execution or reset the guard.
    #[serde(default)]
    pub repeat_finalization: bool,
    /// At most two host read-loop reviews, persisted across checkpoint recovery.
    #[serde(default)]
    pub read_loop_reviews: Vec<serde_json::Value>,
    pub repeated: ConsecutiveCalls,
    pub blocked: Option<StopReason>,
    pub answer: Option<String>,
    #[serde(default)]
    pub failure_finalization_attempted: bool,
    #[serde(default)]
    pub model_corrections: u8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_correction: Option<crate::ModelCorrection>,
    /// Only a newly opened, one-response correction can close its own recovery
    /// episode. Existing tool recovery and all accumulated budgets remain intact.
    #[serde(default)]
    pub model_correction_started_recovery: bool,
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
            recovery_tokens: 0,
            recovery_token_limit: 0,
            recovery_active: false,
            recovery_accounting_initialized: true,
            usage: Usage {
                input_tokens: Some(0),
                output_tokens: Some(0),
                cache_read_tokens: Some(0),
                cache_write_tokens: Some(0),
            },
            unreported_requests: 0,
            successful_executions: 0,
            input_corrections: 0,
            input_recovery_targets: BTreeSet::new(),
            input_recovery_started: false,
            service_recoveries: 0,
            sandbox_corrections: 0,
            sandbox_recovery_targets: vec![],
            sandbox_recovery_started: false,
            repeat_finalization: false,
            read_loop_reviews: vec![],
            repeated: ConsecutiveCalls::new(repeat_limit)?,
            blocked: None,
            answer: None,
            failure_finalization_attempted: false,
            model_corrections: 0,
            model_correction: None,
            model_correction_started_recovery: false,
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
