// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! User-facing projections. Original diagnostics remain in the execution trace.
use serde_json::{Value, json};

pub(crate) fn notice(code: &str) -> Value {
    let (code, message) = match code {
        "capability_permission_required" => (
            code,
            "目前没有获得完成这项任务所需的技能权限，请联系管理员开放相应技能。已完成内容已保留。",
        ),
        "service_unavailable" => (
            code,
            "所需能力暂时不可用，暂时无法完成这项任务。已完成内容已保留。",
        ),
        "result_pending_verification" => (
            code,
            "已有任务的最终结果暂时无法确认。已完成内容已保留，无需重复提交。",
        ),
        "task_scope_limited" => (
            code,
            "当前能力暂时无法处理完整任务，请缩小未完成部分的范围。已完成内容已保留。",
        ),
        "task_processing_limited" => (
            code,
            "这次任务暂时未能完成，已保存的内容会保留。你可以继续完成未完成的部分。",
        ),
        _ => (
            "capability_limited",
            "目前可用的能力暂时无法完成这项任务，已完成内容已保留。可联系管理员检查所需技能与权限。",
        ),
    };
    json!({"code":code,"message":message})
}
fn selected(value: &Value) -> Value {
    notice(value["code"].as_str().unwrap_or("capability_limited"))
}
pub(crate) fn event(mut payload: Value) -> Option<Value> {
    match payload["type"].as_str() {
        Some(
            "execution_exception"
            | "input_rejected"
            | "upstream_error"
            | "tool_output"
            | "mcp_background_task"
            | "mcp_background_retry"
            | "execution_recovery_budget",
        ) => None,
        Some("recoverable") => {
            payload["message"] = notice("result_pending_verification")["message"].clone();
            Some(payload)
        }
        Some("failed" | "error") => {
            let public = selected(&payload["public_notice"]);
            Some(
                json!({"type":payload["type"],"status":"failed","message":public["message"],"public_notice":public,
                "artifacts":payload.get("artifacts").cloned().unwrap_or_else(||json!([]))}),
            )
        }
        Some("tool_progress") => {
            payload["message"] = json!("正在处理任务，已完成内容会保留。");
            Some(payload)
        }
        Some("complete") => {
            response(&mut payload["response"]);
            Some(payload)
        }
        _ => Some(payload),
    }
}
fn response(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("model_error");
        if let Some(public) = object.get_mut("public_notice") {
            *public = selected(public);
        }
    }
}
pub(crate) fn history(value: &mut Value) {
    for field in ["turns", "active_turns"] {
        if let Some(turns) = value[field].as_array_mut() {
            for turn in turns {
                if turn["run_status"] == "failed" || turn["run_status"] == "recoverable" {
                    let public = selected(&turn["response"]["public_notice"]);
                    turn["run_error"] = public["message"].clone();
                    if turn["response"].is_object() {
                        turn["response"]["public_notice"] = public;
                    }
                }
                response(&mut turn["response"]);
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/public_chat.rs"]
mod tests;
