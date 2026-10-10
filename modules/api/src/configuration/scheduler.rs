// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Management adapter for the shared durable scheduler.
use super::ConfigurationState;
use crate::errors::{ApiError, storage};
use axum::{Json, body::Bytes, extract::State, http::StatusCode};
use exoharness_contracts::{capability::CapabilitySpec, scheduler::SchedulerConfiguration};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

async fn response(
    state: ConfigurationState,
    config: SchedulerConfiguration,
) -> Result<Json<Value>, ApiError> {
    let capabilities = if let Some(registry) = state.registry {
        tokio::task::spawn_blocking(move || {
            registry
                .0
                .capabilities(&RegistryAccess::Administrator, 1000)
        })
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "scheduler_worker_failed",
                "调度配置处理暂时不可用",
            )
        })??
    } else {
        vec![]
    };
    let mut result = serde_json::to_value(config).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "scheduler_encoding",
            "无法读取调度配置",
        )
    })?;
    result["mcp_capabilities"] = json!(capabilities.into_iter().filter_map(|cap| {
        if let CapabilitySpec::McpServer(spec) = cap.manifest.spec {
            Some(json!({"id":cap.id,"name":cap.manifest.name,"enabled":cap.enabled,"capability_max_concurrency":spec.limits.max_concurrency}))
        } else {None}
    }).collect::<Vec<_>>());
    Ok(Json(result))
}
pub(super) async fn read(State(state): State<ConfigurationState>) -> Result<Json<Value>, ApiError> {
    let config = storage(state.store.clone(), |s| s.scheduler_configuration()).await?;
    response(state, config).await
}
pub(super) async fn save(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let text = std::str::from_utf8(&body).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_scheduler_json",
            "调度配置必须使用 UTF-8",
        )
    })?;
    let raw = exoharness_contracts::strict_json::parse(text).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_scheduler_json",
            "请输入有效且没有重复字段的调度配置",
        )
    })?;
    let config: SchedulerConfiguration = serde_json::from_value(raw).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_scheduler_configuration",
            "调度配置字段或类型不正确",
        )
    })?;
    config.validate().map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_scheduler_configuration",
            "并发上限、资源池或关联关系不正确",
        )
    })?;
    if !config.mcp_resource_assignments.is_empty() {
        let registry = state.registry.clone().ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "mcp_not_registered",
            "资源池只能关联已登记的 MCP",
        ))?;
        let ids: Vec<String> = config.mcp_resource_assignments.keys().cloned().collect();
        let valid = tokio::task::spawn_blocking(move || {
            ids.iter().all(|id| {
                registry
                    .0
                    .capability(&RegistryAccess::Administrator, id)
                    .is_ok_and(|cap| matches!(cap.manifest.spec, CapabilitySpec::McpServer(_)))
            })
        })
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "scheduler_worker_failed",
                "调度配置处理暂时不可用",
            )
        })?;
        if !valid {
            return Err(ApiError(
                StatusCode::BAD_REQUEST,
                "mcp_not_registered",
                "资源池只能关联已登记的 MCP",
            ));
        }
    }
    let saved = storage(state.store.clone(), move |s| {
        s.save_scheduler_configuration(config)
    })
    .await?;
    response(state, saved).await
}
