// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Only a trusted before-execution schema receipt permits model correction.
use crate::{EngineState, Receipt, ReceiptState};

pub(crate) fn admit(state: &mut EngineState, receipt: &Receipt) -> bool {
    if receipt.state != ReceiptState::RejectedBeforeExecution
        || receipt.performed_execution
        || receipt.result["_host_input_rejection"] != 1
        || receipt.result["code_executed"] != false
        || state.input_corrections >= 2
    {
        return false;
    }
    state.input_corrections += 1;
    true
}

pub(crate) fn rejected(state: &mut EngineState, tool: &str) {
    if !state.recovery_active {
        state.input_recovery_started = true;
    }
    state.input_recovery_targets.insert(tool.into());
}

pub(crate) fn completed(state: &mut EngineState, tool: &str, performed_execution: bool) {
    let mut resolved = state.input_recovery_targets.remove(tool);
    if performed_execution && !state.input_recovery_targets.is_empty() {
        // A different authorized interface can replace an unsubmitted invalid
        // call. Confirmed successful execution is progress; loading schemas or
        // inspecting local state alone does not settle an unrelated rejection.
        state.input_recovery_targets.clear();
        resolved = true;
    }
    if resolved
        && state.input_recovery_targets.is_empty()
        && state.input_recovery_started
        && state.unreported_requests == 0
    {
        // Only close the attributable input-repair episode. Usage, correction
        // counts and the Run-wide budget remain cumulative across episodes.
        state.recovery_active = false;
        state.input_recovery_started = false;
    }
}
