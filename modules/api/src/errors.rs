// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use exoharness_storage_postgres::{PostgresStore, StoreError};
use serde_json::json;

pub struct ApiError(pub StatusCode, pub &'static str, pub &'static str);
impl From<exoharness_skills::SkillError> for ApiError {
    fn from(error: exoharness_skills::SkillError) -> Self {
        match error {
            exoharness_skills::SkillError::Store(error) => error.into(),
            exoharness_skills::SkillError::Io => Self(
                StatusCode::SERVICE_UNAVAILABLE,
                "skill_store_unavailable",
                "Skill内容存储暂时不可用，未执行代码",
            ),
            exoharness_skills::SkillError::Validation(_) => Self(
                StatusCode::BAD_REQUEST,
                "invalid_skill_package",
                "Skill内容或版本校验未通过，未执行代码",
            ),
        }
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.0,
            Json(json!({"error":{"code":self.1,"message":self.2},"resumable":false})),
        )
            .into_response()
    }
}
impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        match error {
            StoreError::NotFound => Self(
                StatusCode::NOT_FOUND,
                "not_found",
                "当前用户范围内未找到记录",
            ),
            StoreError::NotAuthorized => Self(StatusCode::FORBIDDEN, "not_authorized", "未获授权"),
            StoreError::CapacityWaiting => Self(
                StatusCode::CONFLICT,
                "capacity_waiting",
                "执行资源正在使用，请等待排队",
            ),
            StoreError::Conflict => Self(
                StatusCode::CONFLICT,
                "state_conflict",
                "状态已变化，请刷新后使用最新版本",
            ),
            StoreError::OutcomeUnknown => Self(
                StatusCode::CONFLICT,
                "outcome_unknown",
                "调用结果尚待核对，不能重复执行",
            ),
            StoreError::RepeatedCall => Self(
                StatusCode::CONFLICT,
                "repeated_call",
                "连续相同调用达到限制",
            ),
            StoreError::MigrationRequired => Self(
                StatusCode::CONFLICT,
                "migration_required",
                "旧执行记录需要显式迁移，不能直接恢复",
            ),
            StoreError::MissingExecution => Self(
                StatusCode::CONFLICT,
                "missing_execution",
                "未完成实际执行，不能标记成功",
            ),
            StoreError::Invalid(_) => Self(
                StatusCode::BAD_REQUEST,
                "invalid_input",
                "参数不符合执行契约",
            ),
            StoreError::Connection | StoreError::Database | StoreError::Decode => Self(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_unavailable",
                "持久化服务暂时不可用，未确认提交",
            ),
        }
    }
}

/// Never run the synchronous driver on the Tokio request/stream worker.
pub async fn storage<T, F>(store: PostgresStore, work: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(PostgresStore) -> exoharness_storage_postgres::Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(move || work(store))
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "storage_worker_failed",
                "持久化工作线程中断",
            )
        })?
        .map_err(Into::into)
}
