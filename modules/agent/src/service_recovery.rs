// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Replan only when the host confirms no business operation was submitted.
use crate::{EngineState, Receipt, ReceiptState};

pub(crate) fn admit(state: &mut EngineState, receipt: &Receipt) -> bool {
    if receipt.state != ReceiptState::RejectedBeforeExecution
        || receipt.performed_execution
        || receipt.result["_host_service_unavailable"] != 1
        || receipt.result["code_executed"] != false
        || state.service_recoveries >= 2
    {
        return false;
    }
    state.service_recoveries += 1;
    true
}
