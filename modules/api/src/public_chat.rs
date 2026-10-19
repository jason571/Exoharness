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
// Ordinary service progress belongs in chat. Diagnostic/credential messages
// remain in the original journal, even if a service emits them as progress.
fn progress_message(value: &Value) -> &str {
    let Some(message) = value.as_str().map(str::trim).filter(|message| {
        !message.is_empty() && message.len() <= 4096 && !message.chars().any(char::is_control)
    }) else {
        return "正在处理...";
    };
    let lower = message.to_lowercase();
    if lower.starts_with("error")
        || lower.starts_with('{')
        || lower.starts_with("[{")
        || [
            "traceback",
            "exception",
            "runtimeerror",
            "stack trace",
            "panic",
            "failed",
            "failure",
            "error:",
            " error",
            "permission denied",
            "unauthorized",
            "forbidden",
            "http status",
            "mcp 503",
            "authorization",
            "bearer ",
            "api_key",
            "apikey",
            "password",
            "secret",
            "access_token",
            "token=",
            "token:",
            "signature=",
            "http://",
            "https://",
            "file://",
            "/home/",
            "/mnt/",
            "/tmp/",
            "/usr/",
            ":\\",
            "失败",
            "异常",
            "报错",
            "结果未知",
            "权限",
            "拒绝",
            "执行已停止",
            "查看追踪",
        ]
        .iter()
        .any(|pattern| lower.contains(pattern))
    {
        "正在处理..."
    } else {
        message
    }
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
            | "mcp_background_reconciliation"
            | "model_response_correction"
            | "execution_recovery_budget",
        ) => None,
        Some("recoverable") => {
            payload["message"] = notice("result_pending_verification")["message"].clone();
            Some(payload)
        }
        Some("failed" | "error") => {
            let public = selected(&payload["public_notice"]);
            let mut event = json!({"type":payload["type"],"status":"failed","message":public["message"],"public_notice":public,
                "artifacts":payload.get("artifacts").cloned().unwrap_or_else(||json!([]))});
            if payload["response"]["partial_result"] == true {
                response(&mut payload["response"]);
                event["response"] = payload["response"].clone();
            }
            Some(event)
        }
        Some("tool_progress") => {
            // Project the progress contract only; optional private extensions
            // must not bypass the diagnostic event boundary.
            Some(
                json!({"type":"tool_progress", "tool_call_id":payload["tool_call_id"],
                "progress":payload["progress"], "total":payload["total"],
                "message":progress_message(&payload["message"])}),
            )
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
