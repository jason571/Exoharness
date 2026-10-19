// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Measured Run progress, without estimating or rewriting source context.
use crate::{EnginePolicy, EngineState};
use exoharness_contracts::model::Message;
use serde_json::json;

pub(super) fn guidance(state: &EngineState, policy: &EnginePolicy) -> Option<Message> {
    let spent = state.reported_tokens;
    let limit = policy.max_reported_tokens;
    if policy.max_model_calls <= 1 || u128::from(spent) * 10 < u128::from(limit) {
        return None;
    }
    let phase = if u128::from(spent) * 3 >= u128::from(limit) * 2 {
        "Prioritize completing and delivering confirmed work. Do not spend the remaining budget expanding background research or repeating completed checks. If an essential fact cannot be verified, clearly qualify or omit that claim rather than invent it. Never claim a file exists before a successful delivery receipt."
    } else if u128::from(spent) * 3 >= u128::from(limit) {
        "A substantial part of this Run budget is already used. If the task requests a generated output, move from background collection to authorized creation, validation and delivery using confirmed evidence. Further retrieval must resolve a concrete essential gap; an unverified newest reporting period can be labeled unavailable while using the latest confirmed period."
    } else {
        "Reserve most of the remaining Run budget for the requested execution, validation and delivery. Review whether existing evidence is sufficient before another background lookup. For a generated deliverable, a focused, dated and qualified result is preferable to exhaustive preparation with no file delivered."
    };
    Some(Message::RuntimeData {
        text: json!({
            "host_run_budget": {
                "reported_tokens_used": spent,
                "reported_token_limit": limit,
                "reported_tokens_remaining": limit.saturating_sub(spent),
                "model_requests_used": state.model_calls,
                "model_requests_remaining": policy.max_model_calls.saturating_sub(state.model_calls),
                "unreported_requests": state.unreported_requests,
                "usage_complete": state.unreported_requests == 0,
                "guidance": phase
            },
            "authority": "Host accounting guidance only. This does not change tool schemas, permissions, task requirements, retry limits or the Run budget. Reported remaining tokens do not predict the exact cost of the next request."
        }).to_string(),
    })
}
