// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Distinguish instructions, dependency evidence, and executable Tool grants.
use super::RegistryState;
use crate::errors::ApiError;
use axum::http::StatusCode;
use exoharness_skill_runtime::RuntimeError;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

pub(super) fn error(error: RuntimeError) -> ApiError {
    match error {
        RuntimeError::Store(error) => error.into(),
        RuntimeError::Skill(error) => error.into(),
        RuntimeError::Conflict => ApiError(
            StatusCode::CONFLICT,
            "state_conflict",
            "Skill版本已变化，请刷新后准备当前版本",
        ),
        RuntimeError::Permission => ApiError(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "Skill未启用或尚未获授权；未执行上传代码",
        ),
        RuntimeError::Declaration => ApiError(
            StatusCode::BAD_REQUEST,
            "unsupported_runtime_declaration",
            "运行依赖声明无法核验；未执行或安装上传代码",
        ),
        _ => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "runtime_check_unavailable",
            "隔离运行环境暂不可用；不会回退宿主机执行",
        ),
    }
}
pub(super) async fn view(
    state: &RegistryState,
    id: String,
    revision: i64,
    allowed: bool,
) -> Result<Value, ApiError> {
    if let Some(runtime) = &state.runtime {
        let status = runtime
            .status(RegistryAccess::Administrator, id.clone(), revision)
            .await
            .map_err(error)?;
        let message = if !allowed {
            "技能未启用或声明的权限尚未允许"
        } else if !status.scripts_present {
            "技能说明可读取，不需要脚本运行环境"
        } else if status.environment_ready {
            "脚本环境已核验；实际执行仍须独立Tool与Run授权"
        } else {
            "技能说明可读取；脚本环境尚未核验或缺少依赖"
        };
        Ok(
            json!({"status":if allowed {"ready"} else {"blocked"},"message":message,
            "capability_id":id,"revision":revision,"runtime_ready":allowed,
            "instructions_ready":true,"script_execution_ready":false,
            "scripts_present":status.scripts_present,"environment_ready":status.environment_ready,"evidence":status.evidence}),
        )
    } else {
        Ok(json!({"status":if allowed {"ready"}else{"blocked"},
            "message":if allowed {"技能说明可读取；脚本隔离运行服务未配置"}else{"技能未启用或声明的权限尚未允许"},
            "capability_id":id,"revision":revision,"runtime_ready":allowed,
            "instructions_ready":true,"environment_ready":false,"script_execution_ready":false}))
    }
}
