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
