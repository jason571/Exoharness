// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Durable invocation admission is separate from model and protocol adapters.
use super::text_driver::TextDriver;
use crate::errors::storage;
use exoharness_agent::{DurableReference, Receipt, ReceiptState, StopReason};
use exoharness_contracts::{
    execution::InvocationIntent, invocation::InvocationState, model::ToolCall,
};
use serde_json::json;
#[path = "checkpoint.rs"]
mod checkpoint;
#[path = "mcp_admission.rs"]
mod mcp_admission;

async fn event(driver: &TextDriver, payload: serde_json::Value) -> Result<(), StopReason> {
    let (owner, session, run) = (
        driver.owner.clone(),
        driver.session.clone(),
        driver.run.clone(),
    );
    storage(driver.state.store.clone(), move |s| {
        s.chat_event(&owner, &session, &run, payload)
    })
    .await
    .map_err(|_| StopReason::PersistenceFailed)
}
pub(super) async fn invoke(
    driver: &mut TextDriver,
    key: &str,
    call: &ToolCall,
) -> Result<Receipt, StopReason> {
    let tools = driver
        .state
        .tools
        .clone()
        .ok_or(StopReason::InvalidProposal)?;
    let binding = driver
        .catalog
        .as_ref()
        .and_then(|c| c.binding(&call.name))
        .ok_or(StopReason::InvalidProposal)?;
    let native_workspace = binding.capability == exoharness_tools::SANDBOX_ID;
    // Refresh the assistant before every side effect; changing its access or
    // revision cannot silently reuse a previous grant.
    let assistants = driver.state.assistants.clone();
    let (own, sid, rid) = (
        driver.owner.clone(),
        driver.session.clone(),
        driver.run.clone(),
    );
    let frozen = storage(driver.state.store.clone(), move |s| s.run(&own, &sid, &rid))
        .await
        .map_err(|_| StopReason::PersistenceFailed)?
        .snapshot;
    let live = tokio::task::spawn_blocking(move || {
        assistants.refresh().ok()?;
        assistants.detail(&frozen.assistant_id).filter(|p| {
            p["enabled"] == true && p["active_revision"].as_u64() == Some(frozen.assistant_revision)
        })
    })
    .await
    .map_err(|_| StopReason::PersistenceFailed)?;
    if live.is_none() {
        driver.failure = Some("助理配置或授权已改变；本次未执行，请开启新的运行");
        return Err(StopReason::InputRejected);
    }
    let id = format!("{}:{key}", driver.run);
    let diagnostics =
        exoharness_tools::input_diagnostics(&binding.tool.input_schema, &call.arguments).map_err(
            |_| {
                driver.failure = Some("工具Schema无效；未执行，请检查能力登记");
                StopReason::InputRejected
            },
        )?;
    if let Some(diagnostics) = diagnostics {
        let result = json!({"isError":true,"_host_input_rejection":1,"code_executed":false,
            "diagnostics":diagnostics,
            "correction":"Correct only the invalid input using this tool's live Schema. Preserve exact authored data and successful results. Do not delete authored data merely to pass validation. If inline data and an existing resource select incompatible modes, use an authorized staging/edit tool before execution. Remaining calls in this proposed batch have not been submitted; do not repeat completed calls."});
        event(driver,json!({"type":"input_rejected","tool_call_id":id,"tool":call.name,
            "capability":binding.capability,"revision":binding.revision,"arguments":call.arguments,"result":result})).await?;
        return Ok(Receipt {
            invocation_id: id,
            state: ReceiptState::RejectedBeforeExecution,
            performed_execution: false,
            result,
            checkpoint: None,
        });
    }
    let intent = InvocationIntent {
        id: id.clone(),
        logical_key: key.into(),
        capability_id: binding.capability.clone(),
        capability_revision: binding.revision,
        tool: call.name.clone(),
        arguments: call.arguments.clone(),
    };
    let (owner, session, run) = (
        driver.owner.clone(),
        driver.session.clone(),
        driver.run.clone(),
    );
    let record = storage(driver.state.store.clone(), move |s| {
        s.prepare_invocation(&owner, &session, &run, &intent)
    })
    .await
    .map_err(|_| StopReason::PersistenceFailed)?;
    if record.state == InvocationState::Succeeded {
        let result = record.result.ok_or(StopReason::PersistenceFailed)?;
        let checkpoint = workspace_checkpoint(native_workspace, &id, &result)
            .or_else(|| checkpoint::external(driver.catalog.as_ref()?, &binding, &id, &result));
        return Ok(Receipt {
            invocation_id: id,
            state: ReceiptState::Completed,
            performed_execution: result["_host_execution"] == true,
            result,
            checkpoint,
        });
    }
    if record.state != InvocationState::Prepared {
        return Err(StopReason::OutcomeUnknown);
    }
    mcp_admission::claim(driver, &tools, &binding.capability, &id).await?;
    event(
        driver,
        json!({"type":"tool_call","tool_call_id":id,"tool":call.name}),
    )
    .await?;
    let (tx, mut rx) = tokio::sync::watch::channel(None);
    let mut catalog = driver.catalog.take().ok_or(StopReason::InvalidProposal)?;
    let mut progress_failure = None;
    let outcome = {
        let operation = tools.dispatch(
            &mut catalog,
            &driver.owner,
            &driver.session,
            &driver.run,
            &id,
            call,
            driver.cancel.clone(),
            tx,
        );
        let mut stop = driver.state.stop.clone();
        let mut progress_open = true;
        tokio::pin!(operation);
        loop {
            tokio::select! {biased;
                result=&mut operation=>break result,
                _=stop.changed()=>{driver.cancel.cancel();},
                changed=rx.changed(),if progress_open=>{
                    if changed.is_ok() {
                        let update=rx.borrow_and_update().clone();
                        if let Some(update)=update
                            && let Err(error)=event(driver,json!({"type":"tool_progress","tool_call_id":id,"progress":update.progress,"total":update.total,"message":update.message})).await {
                                // Cancellation may commit between receipt of progress
                                // and its journal write. Never drop submitted work:
                                // drain the adapter and persist its authoritative receipt.
                                progress_failure=Some(error);
                                driver.cancel.cancel();
                                progress_open=false;
                        }
                    } else {
                        progress_open=false;
                    }
                }
            }
        }
    };
    // The dispatch borrow must end before putting its mutable catalog back.
    driver.catalog = Some(catalog);
    let (state, receipt_state, mut value, executed, artifacts) = match outcome {
        Ok(result) if !result.failed => (
            InvocationState::Succeeded,
            ReceiptState::Completed,
            result.value,
            result.executed,
            result.artifacts,
        ),
        Ok(result) => (
            InvocationState::FailedAfterExecution,
            ReceiptState::FailedAfterExecution,
            result.value,
            result.executed,
            result.artifacts,
        ),
        Err(exoharness_tools::ToolError::CorrectableInput(message)) => (
            InvocationState::RejectedBeforeExecution,
            ReceiptState::RejectedBeforeExecution,
            json!({"isError":true,"_host_input_rejection":1,"code_executed":false,
                "diagnostics":{"message":message},"correction":"This host-validated input was rejected before execution. Correct only the stated input using focused source context, the live schema and the current revision; preserve successful work. Remaining calls in the proposed batch were not submitted. Do not repeat completed calls."}),
            false,
            vec![],
        ),
        Err(exoharness_tools::ToolError::MissingOutput(path)) if native_workspace => (
            InvocationState::RejectedBeforeExecution,
            ReceiptState::RejectedBeforeExecution,
            json!({"isError":true,"_host_input_rejection":1,"code_executed":false,
                "diagnostics":{"missing_output":path},"correction":"This publication selection was rejected before publishing any files. Select the intended existing file from the saved workspace inventory and use workspace_publish; do not rerun successful code."}),
            false,
            vec![],
        ),
        Err(exoharness_tools::ToolError::Unknown) => (
            InvocationState::OutcomeUnknown,
            ReceiptState::OutcomeUnknown,
            json!({"isError":true,"message":"结果未知，不会重复执行"}),
            false,
            vec![],
        ),
        Err(error) => (
            InvocationState::RejectedBeforeExecution,
            ReceiptState::RejectedBeforeExecution,
            json!({"isError":true,"message":error.to_string()}),
            false,
            vec![],
        ),
    };
    value["_host_execution"] = json!(executed);
    let (owner, session, run, resolve_id, result) = (
        driver.owner.clone(),
        driver.session.clone(),
        driver.run.clone(),
        id.clone(),
        value.clone(),
    );
    storage(driver.state.store.clone(), move |s| {
        s.resolve_invocation(&owner, &session, &run, &resolve_id, state, Some(result))
    })
    .await
    .map_err(|_| StopReason::PersistenceFailed)?;
    if let Some(error) = progress_failure {
        return Err(error);
    }
    event(driver,json!({"type":"tool_result","tool_call_id":id,"tool":call.name,"is_error":receipt_state!=ReceiptState::Completed})).await?;
    driver.tool_calls.push(call.name.clone());
    driver.artifacts.extend(artifacts);
    if receipt_state != ReceiptState::Completed && value["_host_input_rejection"] != 1 {
        driver.failure = Some(match receipt_state {
            ReceiptState::RejectedBeforeExecution => {
                "当前步骤在执行前被拒绝；此前已完成的步骤和结果已保留，请查看追踪中的具体原因"
            }
            ReceiptState::FailedAfterExecution => {
                "能力执行失败，具体参数与返回结果已保存到追踪记录"
            }
            ReceiptState::OutcomeUnknown => {
                "能力调用结果尚未确认，记录已保存；不会重复提交未知结果"
            }
            ReceiptState::Completed => unreachable!(),
        });
    }
    let checkpoint = (receipt_state == ReceiptState::Completed)
        .then(|| {
            workspace_checkpoint(native_workspace, &id, &value)
                .or_else(|| checkpoint::external(driver.catalog.as_ref()?, &binding, &id, &value))
        })
        .flatten();
    Ok(Receipt {
        invocation_id: id,
        state: receipt_state,
        result: value,
        performed_execution: executed,
        checkpoint,
    })
}

// Only exact native receipts already committed above may replace source data.
// External MCP metadata cannot opt into this verified checkpoint path.
fn workspace_checkpoint(
    native: bool,
    invocation: &str,
    value: &serde_json::Value,
) -> Option<DurableReference> {
    let durable_read = native
        && value["file"]["sha256"].is_string()
        && value["file"]["path"].is_string()
        && value["revision"].is_u64()
        && value["content"].is_string();
    if !native || (value["_workspace_checkpoint"] != true && !durable_read) {
        return None;
    }
    let mut summary = value.clone();
    summary.as_object_mut()?.remove("_workspace_checkpoint");
    if durable_read {
        summary["_durable_read_body"] = json!(true);
    }
    super::publication_checkpoint::project(&mut summary, value);
    if let Some(files) = summary
        .get_mut("files")
        .and_then(serde_json::Value::as_array_mut)
    {
        let count = files.len();
        files.truncate(64);
        summary["file_count"] = json!(count);
        summary["file_inventory_truncated"] = json!(count > 64);
    }
    if let Some(execution) = summary.get_mut("execution") {
        for key in ["stdout", "stderr"] {
            if let Some(text) = execution[key].as_str()
                && text.chars().count() > 4000
            {
                let tail = text
                    .chars()
                    .rev()
                    .take(4000)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect::<String>();
                execution[key] = json!(tail);
                execution["checkpoint_log_truncated"] = json!(true);
            }
        }
    }
    Some(DurableReference {
        reference: format!("urn:exoharness:invocation:{invocation}"),
        summary,
    })
}
