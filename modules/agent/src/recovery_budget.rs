// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Run-wide, persisted recovery accounting; no unreported request is free.
use crate::{EnginePolicy, EngineState, StopReason};
use exoharness_contracts::model::{ChatRequest, ToolCall, Usage};
use exoharness_core::context::JournalEntry;

pub(super) fn initialize(state: &mut EngineState, policy: &EnginePolicy) {
    if state.recovery_token_limit == 0 {
        state.recovery_token_limit = policy.max_recovery_tokens;
    }
    state.recovery_token_limit = state.recovery_token_limit.min(policy.max_recovery_tokens);
    if !state.recovery_accounting_initialized {
        // Older checkpoints have no trustworthy attribution for past retries.
        if state.input_corrections > 0
            || state.service_recoveries > 0
            || state.sandbox_corrections > 0
        {
            state.recovery_active = true;
            state.recovery_tokens = state.recovery_token_limit;
        }
        state.recovery_accounting_initialized = true;
    }
}
pub(super) fn unchanged(state: &EngineState, call: &ToolCall) -> bool {
    let previous = state.entries.iter().rev().find_map(|entry| match entry {
        JournalEntry::ToolExchange {
            tool,
            arguments,
            result,
            ..
        } => Some((tool, arguments, result)),
        _ => None,
    });
    previous.is_some_and(|(tool, arguments, result)| {
        tool == &call.name
            && ((result["code_executed"] == false
                && (result["_host_service_unavailable"] == 1
                    || result["_host_input_rejection"] == 1 && arguments == &call.arguments))
                || (result["_host_sandbox_failure"] == 1
                    && call.arguments["workspace_id"].is_string()
                    && arguments.get("workspace_id").unwrap_or(&result["workspace_id"])
                        == &call.arguments["workspace_id"]
                    && arguments["entrypoint"] == call.arguments["entrypoint"]
                    && arguments["runtime"] == call.arguments["runtime"]))
    })
}
pub(super) fn reserve(
    state: &mut EngineState,
    request: &mut ChatRequest,
    output_cap_available: bool,
) -> Result<u64, StopReason> {
    if !state.recovery_active {
        return Ok(0);
    }
    if !output_cap_available {
        return Err(StopReason::RecoveryTokenBudget);
    }
    // UTF-8 bytes conservatively bound input tokens for this text-only contract.
    // Reserve adapter overhead too; bytes/4 is not an exact tokenizer count.
    let input_bound = serde_json::to_vec(request)
        .map_err(|_| StopReason::InvalidProposal)?
        .len() as u64
        + 1024;
    let remaining = state
        .recovery_token_limit
        .saturating_sub(state.recovery_tokens);
    if remaining <= input_bound {
        return Err(StopReason::RecoveryTokenBudget);
    }
    request.output_token_limit = request.output_token_limit.min(remaining - input_bound);
    let reserved = input_bound.saturating_add(request.output_token_limit);
    state.recovery_tokens = state.recovery_tokens.saturating_add(reserved);
    Ok(reserved)
}
pub(super) fn settle(state: &mut EngineState, reserved: u64, usage: &Usage) {
    if reserved == 0 {
        return;
    }
    if let (Some(input), Some(output)) = (usage.input_tokens, usage.output_tokens) {
        state.recovery_tokens = state
            .recovery_tokens
            .saturating_sub(reserved)
            .saturating_add(input.saturating_add(output));
    }
}
