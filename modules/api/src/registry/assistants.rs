// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Original assistant UI projection; canonical registry kinds remain unchanged.
use super::{RegistryState, reads};
use crate::errors::ApiError;
use axum::{Json, extract::State};
use serde_json::{Value, json};
pub(super) async fn capabilities(
    state: State<RegistryState>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let sandbox_ready = state.runtime.is_some();
    let Json(mut records) = reads::overview(state).await?;
    records.retain(|r| r.pointer("/manifest/scope/kind") == Some(&json!("global")));
    for record in &mut records {
        if record.pointer("/manifest/spec/kind") == Some(&json!("tool_node")) {
            record["manifest"]["spec"]["kind"] = json!("tool");
        }
    }
    records.push(json!({
        "id":exoharness_tools::SANDBOX_ID,"revision":1,"enabled":true,
        "manifest":{"name":"任务沙盒","description":"原生隔离执行：Python、Shell、JavaScript、TypeScript；支持任务文件、局部修改及最终文件交付。授予 Skill 不会自动授予此执行权限。","scope":{"kind":"global"},"spec":{"kind":"tool"}},
        "availability":{"status":if sandbox_ready {"available"} else {"unavailable"},"available":sandbox_ready,"runtime_ready":sandbox_ready,"instructions_ready":false,"script_execution_ready":sandbox_ready,"blocked_permissions":[],"message":if sandbox_ready {"原生沙盒已接入；须显式授予助理"} else {"原生沙盒运行时尚未就绪"}}
    }));
    Ok(Json(records))
}
