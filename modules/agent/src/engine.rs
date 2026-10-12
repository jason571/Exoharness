// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! One provider-neutral serial loop. API, MCP and sandbox adapters are ports.
use crate::{
    Driver, EnginePolicy, EngineState, PendingCall, ReceiptState, RunOutcome, StopReason,
    project_request,
};
use exoharness_contracts::model::{Finish, Tool};
use exoharness_core::context::JournalEntry;
use std::collections::{BTreeSet, HashSet};

async fn stop(driver: &mut dyn Driver, state: &mut EngineState, reason: StopReason) -> RunOutcome {
    state.blocked = Some(reason.clone());
    if !persist(driver, state).await {
        return RunOutcome::Stopped(StopReason::PersistenceFailed);
    }
    RunOutcome::Stopped(reason)
}
async fn persist(driver: &mut dyn Driver, state: &mut EngineState) -> bool {
    if driver.save(state).await.is_err() {
        state.blocked = Some(StopReason::PersistenceFailed);
        false
    } else {
        true
    }
}

pub async fn run(
    driver: &mut dyn Driver,
    state: &mut EngineState,
    policy: &EnginePolicy,
    system: &str,
    selected: &BTreeSet<String>,
    tools: &[Tool],
) -> RunOutcome {
    if state.version != 1 {
        return stop(driver, state, StopReason::InvalidProposal).await;
    }
    if let Some(answer) = &state.answer {
        return RunOutcome::Finished(answer.clone());
    }
    if let Some(reason) = &state.blocked {
        return RunOutcome::Stopped(reason.clone());
    }
    if state.in_flight_model.is_some() {
        return stop(driver, state, StopReason::ModelIncomplete).await;
    }
    if !policy.valid() {
        return stop(driver, state, StopReason::InvalidProposal).await;
    }
    loop {
        if driver.cancelled() {
            return stop(driver, state, StopReason::Cancelled).await;
        }
        if state.reported_tokens >= policy.max_reported_tokens {
            return stop(driver, state, StopReason::TokenBudget).await;
        }
        if let Some(mut pending) = state.pending.first().cloned() {
            if !pending.admitted {
                // A committed workflow receipt may have changed the allowed
                // next step after this batch was proposed. Never submit stale
                // dependents against the old schema set.
                if !driver
                    .request_tools(&state.entries, tools)
                    .iter()
                    .any(|tool| tool.name == pending.call.name)
                {
                    return stop(driver, state, StopReason::InvalidProposal).await;
                }
                if state.tool_calls >= policy.max_tool_calls {
                    return stop(driver, state, StopReason::ToolBudget).await;
                }
                if !state
                    .repeated
                    .admit(&pending.call.name, &pending.call.arguments)
                {
                    if state.repeat_finalization {
                        return stop(driver, state, StopReason::RepeatedCall).await;
                    }
                    state.repeat_finalization = true;
                    state.entries.push(JournalEntry::ToolExchange {
                        invocation_id: format!("{}:repeat-rejected", pending.key),
                        tool: pending.call.name,
                        arguments: pending.call.arguments,
                        result: serde_json::json!({
                            "isError": true,
                            "code": "repeated_call_not_submitted",
                            "performed_execution": false,
                            "recovery": "This proposal was not submitted. No further tools are available in this run. Use existing receipts to give a concise, helpful final response in the user's language. Deliver any already confirmed result; if a background task was submitted but not confirmed terminal, say that its completion has not yet been verified and retain its task ID for later status lookup. Do not claim generation failed or succeeded without evidence. Do not recommend resubmitting generation. Explain the remaining limitation without exposing internal guard errors."
                        }),
                    });
                    state.pending.clear();
                    if !persist(driver, state).await {
                        return RunOutcome::Stopped(StopReason::PersistenceFailed);
                    }
                    continue;
                }
                state.tool_calls += 1;
                pending.admitted = true;
                state.pending[0] = pending.clone();
                if !persist(driver, state).await {
                    return RunOutcome::Stopped(StopReason::PersistenceFailed);
                }
            }
            let receipt = match driver.invoke(&pending.key, &pending.call).await {
                Ok(receipt) => receipt,
                Err(reason) => return stop(driver, state, reason).await,
            };
            if receipt.state == ReceiptState::OutcomeUnknown {
                return stop(driver, state, StopReason::OutcomeUnknown).await;
            }
            if receipt.invocation_id.is_empty() || state.entries.iter().any(|entry|matches!(entry, JournalEntry::ToolExchange{invocation_id,..} if invocation_id == &receipt.invocation_id)) {
                return stop(driver,state,StopReason::InvalidProposal).await;
            }
            let correctable = crate::input_correction::admit(state, &receipt);
            let recoverable = crate::service_recovery::admit(state, &receipt);
            let sandbox_correctable = crate::sandbox_recovery::admit(state, &receipt);
            state.entries.push(JournalEntry::ToolExchange {
                invocation_id: receipt.invocation_id.clone(),
                tool: pending.call.name,
                arguments: pending.call.arguments,
                result: receipt.result,
            });
            if let Some(checkpoint) = receipt
                .checkpoint
                .filter(|c| !c.reference.trim().is_empty())
            {
                state.entries.push(JournalEntry::Checkpoint {
                    invocation_id: receipt.invocation_id,
                    reference: checkpoint.reference,
                    summary: checkpoint.summary,
                });
            }
            state.pending.remove(0);
            match receipt.state {
                ReceiptState::Completed => {
                    if receipt.performed_execution {
                        state.successful_executions += 1;
                    }
                }
                ReceiptState::RejectedBeforeExecution => {
                    if !correctable && !recoverable {
                        return stop(driver, state, StopReason::InputRejected).await;
                    }
                    // The remaining proposals in this batch were never
                    // submitted. Replan from exact successful receipts and
                    // the rejected call, rather than execute stale dependents.
                    state.pending.clear();
                }
                ReceiptState::FailedAfterExecution => {
                    if !sandbox_correctable {
                        return stop(driver, state, StopReason::ToolFailed).await;
                    }
                    state.pending.clear();
                }
                ReceiptState::OutcomeUnknown => unreachable!(),
            }
            if !persist(driver, state).await {
                return RunOutcome::Stopped(StopReason::PersistenceFailed);
            }
            continue;
        }
        if state.model_calls >= policy.max_model_calls {
            return stop(driver, state, StopReason::ModelBudget).await;
        }
        let active_tools = if state.repeat_finalization {
            vec![]
        } else {
            driver.request_tools(&state.entries, tools)
        };
        let reminded_system;
        let request_system = if state.repeated.at_limit() && !state.repeat_finalization {
            reminded_system = format!(
                "{system}\nHost execution reminder: You have repeated the same tool with identical arguments. Inspect the latest receipt and choose a different permitted action or finish with confirmed results. Do not repeat generation or other side effects. If an existing background job is pending, retain its ID and do not claim it failed merely because it has not completed yet."
            );
            reminded_system.as_str()
        } else {
            system
        };
        let request = match project_request(
            request_system,
            &state.entries,
            selected,
            active_tools.clone(),
            policy.output_token_limit,
        ) {
            Ok(request) => request,
            Err(_) => return stop(driver, state, StopReason::InvalidProposal).await,
        };
        if serde_json::to_vec(&request).map_or(true, |bytes| bytes.len() > policy.max_context_bytes)
        {
            return stop(driver, state, StopReason::ContextBudget).await;
        }
        state.model_calls += 1;
        let key = format!("model-{}", state.model_calls);
        state.in_flight_model = Some(key.clone());
        if !persist(driver, state).await {
            return RunOutcome::Stopped(StopReason::PersistenceFailed);
        }
        let completion = match driver.model(&key, request).await {
            Ok(completion) => completion,
            Err(reason) => return stop(driver, state, reason).await,
        };
        state.in_flight_model = None;
        state.add_usage(&completion.usage);
        if completion.calls.is_empty() {
            if completion.finish != Finish::Stop || completion.text.trim().is_empty() {
                return stop(driver, state, StopReason::ModelIncomplete).await;
            }
            if !state.repeat_finalization && crate::workflow::workflow_required(&state.entries) {
                return stop(driver, state, StopReason::MissingExecution).await;
            }
            if !state.repeat_finalization
                && driver.requires_execution(policy.requires_execution)
                && state.successful_executions == 0
            {
                return stop(driver, state, StopReason::MissingExecution).await;
            }
            state
                .entries
                .push(JournalEntry::Assistant(completion.text.clone()));
            state.answer = Some(completion.text.clone());
            if !persist(driver, state).await {
                state.answer = None;
                return RunOutcome::Stopped(StopReason::PersistenceFailed);
            }
            return RunOutcome::Finished(completion.text);
        }
        let mut ids = HashSet::new();
        if completion.finish != Finish::ToolCalls
            || completion.calls.iter().any(|call| {
                call.id.is_empty()
                    || !ids.insert(&call.id)
                    || !call.arguments.is_object()
                    || !active_tools.iter().any(|tool| tool.name == call.name)
            })
        {
            if state.repeat_finalization {
                return stop(driver, state, StopReason::RepeatedCall).await;
            }
            return stop(driver, state, StopReason::InvalidProposal).await;
        }
        state.pending = completion
            .calls
            .into_iter()
            .enumerate()
            .map(|(index, call)| PendingCall {
                key: format!("{key}-tool-{index}"),
                call,
                admitted: false,
            })
            .collect();
        if !persist(driver, state).await {
            return RunOutcome::Stopped(StopReason::PersistenceFailed);
        }
    }
}
