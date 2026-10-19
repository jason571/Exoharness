// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Replan from a host-confirmed, terminated OCI failure, never an unknown call.
use crate::{EngineState, Receipt, ReceiptState, SandboxRecoveryTarget};
use exoharness_contracts::model::ToolCall;

pub(crate) fn admit(state: &mut EngineState, receipt: &Receipt) -> bool {
    if receipt.state != ReceiptState::FailedAfterExecution
        || !receipt.performed_execution
        || receipt.result["_host_sandbox_failure"] != 1
        || receipt.result["execution"]["cleanup_confirmed"] != true
        || receipt.result["execution"]["status"] != "failed"
        || receipt.result["execution"]["exit_code"]
            .as_i64()
            .is_none_or(|n| n == 0)
        || state.sandbox_corrections >= 2
    {
        return false;
    }
    state.sandbox_corrections += 1;
    true
}

fn target(call: &ToolCall, receipt: &Receipt) -> Option<SandboxRecoveryTarget> {
    let workspace = receipt.result["workspace_id"].as_str()?.to_owned();
    if workspace.is_empty()
        || call
            .arguments
            .get("workspace_id")
            .is_some_and(|w| w.as_str() != Some(&workspace))
    {
        return None;
    }
    let entrypoint = call.arguments["entrypoint"].as_str()?.to_owned();
    let runtime = call.arguments["runtime"].as_str()?.to_owned();
    if entrypoint.is_empty() || runtime.is_empty() {
        return None;
    }
    Some(SandboxRecoveryTarget {
        tool: call.name.clone(),
        workspace,
        entrypoint,
        runtime,
    })
}

pub(crate) fn rejected(state: &mut EngineState, call: &ToolCall, receipt: &Receipt) {
    let Some(target) = target(call, receipt) else {
        // Unattributed or legacy failure cannot open a closeable episode.
        state.sandbox_recovery_started = false;
        return;
    };
    if !state.recovery_active {
        state.sandbox_recovery_started = true;
    }
    if !state.sandbox_recovery_targets.contains(&target) {
        state.sandbox_recovery_targets.push(target);
    }
}

pub(crate) fn completed(state: &mut EngineState, call: &ToolCall, receipt: &Receipt) {
    if receipt.state != ReceiptState::Completed
        || !receipt.performed_execution
        || receipt.result["isError"] == true
        || receipt.result["execution"]["cleanup_confirmed"] != true
        || receipt.result["execution"]["status"] != "completed"
        || receipt.result["execution"]["exit_code"].as_i64() != Some(0)
    {
        return;
    }
    let Some(target) = target(call, receipt) else {
        return;
    };
    let unresolved = state.sandbox_recovery_targets.len();
    state
        .sandbox_recovery_targets
        .retain(|item| item != &target);
    if state.sandbox_recovery_targets.len() < unresolved
        && state.sandbox_recovery_targets.is_empty()
        && state.sandbox_recovery_started
        && state.unreported_requests == 0
        && state.input_recovery_targets.is_empty()
    {
        // The same owned program has now finished successfully. Publication is
        // normal task work; accumulated retry costs and counts remain intact.
        state.recovery_active = false;
        state.sandbox_recovery_started = false;
    }
}
