// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::RegistryState;
use crate::errors::{ApiError, storage};
use axum::{Json, extract::State};
use exoharness_contracts::capability::{CapabilityKind, HealthStatus, PermissionDecision};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

pub(super) async fn records(State(state): State<RegistryState>) -> Result<Json<Value>, ApiError> {
    let records = storage(state.registry.0.clone(), |store| {
        store.capabilities(&RegistryAccess::Administrator, 1000)
    })
    .await?;
    Ok(Json(json!(records)))
}
pub(super) async fn overview(
    State(state): State<RegistryState>,
) -> Result<Json<Vec<Value>>, ApiError> {
    let views = storage(state.registry.0.clone(), |store| {
        store.capability_views(&RegistryAccess::Administrator, 1000)
    })
    .await?;
    let mut items = Vec::new();
    for view in views {
        let blocked: Vec<_> = view
            .permissions
            .iter()
            .filter(|record| record.decision != PermissionDecision::Allow)
            .map(|record| record.permission.clone())
            .collect();
        let content_ready = if view.capability.manifest.kind() == CapabilityKind::Skill {
            let skills = state.skills.clone();
            let id = view.capability.id.clone();
            let revision = view.capability.revision;
            tokio::task::spawn_blocking(move || {
                skills
                    .instruction_view(&RegistryAccess::Administrator, &id)
                    .is_ok_and(|(r, _)| r.revision == revision)
            })
            .await
            .unwrap_or(false)
        } else if view.capability.manifest.kind() == CapabilityKind::McpServer {
            view.health
                .as_ref()
                .is_some_and(|health| health.status == HealthStatus::Healthy)
        } else {
            false
        };
        let available = view.capability.enabled && blocked.is_empty() && content_ready;
        let message = if !view.capability.enabled {
            "能力未启用"
        } else if !blocked.is_empty() {
            "声明的权限尚未全部允许"
        } else if content_ready && view.capability.manifest.kind() == CapabilityKind::McpServer {
            "MCP权限与标准连接验证通过"
        } else if content_ready {
            "技能说明可用；脚本执行须单独授予沙盒工具"
        } else {
            "执行连接或内容尚未就绪"
        };
        let instructions_ready =
            content_ready && view.capability.manifest.kind() == CapabilityKind::Skill;
        let mut value = serde_json::to_value(view.capability).map_err(|_| {
            ApiError(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "configuration_unavailable",
                "能力记录读取失败",
            )
        })?;
        value["availability"] = json!({"status":if available {"available"} else {"unavailable"},"available":available,"runtime_ready":content_ready,"instructions_ready":instructions_ready,"script_execution_ready":false,"health":view.health.map(|health|health.status),"blocked_permissions":blocked,"message":message});
        items.push(value);
    }
    Ok(Json(items))
}
