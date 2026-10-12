// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Narrow ports. Adapters own revision/grant/schema admission and durable claims.
use crate::{EngineState, StopReason};
use async_trait::async_trait;
use exoharness_contracts::model::{ChatRequest, Completion, Tool, ToolCall};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct DurableReference {
    pub reference: String,
    pub summary: Value,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReceiptState {
    Completed,
    RejectedBeforeExecution,
    FailedAfterExecution,
    OutcomeUnknown,
}
#[derive(Debug, Clone)]
pub struct Receipt {
    /// Host-assigned stable identity, independent of a provider tool_call_id.
    pub invocation_id: String,
    pub state: ReceiptState,
    pub result: Value,
    pub performed_execution: bool,
    /// Only a persisted, adapter-verified checkpoint can replace the exchange.
    pub checkpoint: Option<DurableReference>,
}
#[async_trait]
pub trait Driver: Send {
    fn cancelled(&self) -> bool;
    /// Provider-owned catalog projection. Default preserves static adapters.
    fn tools(&self, fallback: &[Tool]) -> Vec<Tool> {
        fallback.to_vec()
    }
    /// Reversible stage projection; static drivers preserve their old behavior.
    fn request_tools(
        &mut self,
        _entries: &[exoharness_core::context::JournalEntry],
        fallback: &[Tool],
    ) -> Vec<Tool> {
        self.tools(fallback)
    }
    /// Selected executable workflows can strengthen, never clear, a caller's
    /// explicit execution requirement.
    fn requires_execution(&self, requested: bool) -> bool {
        requested
    }
    /// Persist exact input/output, use the frozen model revision, classify an
    /// incomplete response and perform at most one physical model request.
    async fn model(&mut self, key: &str, request: ChatRequest) -> Result<Completion, StopReason>;
    /// Check live grant, revision and JSON Schema, then atomically claim this
    /// stable logical key before any external side effect. An existing receipt
    /// is replayed locally; submitted/unknown outcomes must NEVER be resubmitted.
    /// Cancellation must propagate to the adapter, not merely drop its future.
    async fn invoke(&mut self, key: &str, call: &ToolCall) -> Result<Receipt, StopReason>;
    /// Persist host diagnostics to the execution trace, without exposing them in chat.
    async fn record_notice(&mut self, _notice: Value) -> Result<(), StopReason> {
        Ok(())
    }
    /// Commit cursor/counters and appended journal entries atomically using CAS.
    /// Full exchanges belong in the journal, not copied into checkpoint metadata.
    async fn save(&mut self, state: &EngineState) -> Result<(), StopReason>;
}
