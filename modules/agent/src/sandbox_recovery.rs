// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Replan from a host-confirmed, terminated OCI failure, never an unknown call.
use crate::{EngineState, Receipt, ReceiptState};

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
