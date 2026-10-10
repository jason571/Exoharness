// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Explicit preparation checks are separate from side-effect-free import.
use super::RegistryState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_contracts::capability::{CapabilityKind, HealthStatus, PermissionDecision};
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    expected_revision: i64,
    confirm: bool,
    #[serde(default)]
    actor: Option<String>,
}
pub(super) async fn inspect(state: RegistryState, id: String) -> Result<Value, ApiError> {
    let lookup = id.clone();
    let (record, permissions, health) = storage(state.registry.0.clone(), move |s| {
        Ok((
            s.capability(&RegistryAccess::Administrator, &lookup)?,
            s.capability_permissions(&RegistryAccess::Administrator, &lookup)?,
            s.capability_health(&RegistryAccess::Administrator, &lookup)?,
        ))
    })
    .await?;
    let allowed = permissions
        .iter()
        .all(|p| p.decision == PermissionDecision::Allow);
    match record.manifest.kind() {
        CapabilityKind::Skill => {
            let skills = state.skills.clone();
            let lookup = id.clone();
            tokio::task::spawn_blocking(move || {
                skills.instructions(&RegistryAccess::Administrator, &lookup)
            })
            .await
            .map_err(|_| exoharness_mcp::McpError::Configuration)?
            .map_err(ApiError::from)?;
            super::skill_runtime::view(&state, id, record.revision, allowed && record.enabled).await
        }
        CapabilityKind::McpServer => {
            let ready = record.enabled
                && allowed
                && health.is_some_and(|h| h.status == HealthStatus::Healthy);
            Ok(
                json!({"status":if ready{"ready"}else{"blocked"},"message":if ready{"MCP权限及标准连接验证通过；业务调用须由Run授权"}else{"MCP尚未完成启用、权限或连接验证"},"capability_id":id,"revision":record.revision,"runtime_ready":ready}),
            )
        }
        CapabilityKind::ToolNode => Ok(
            json!({"status":"blocked","message":"独立Tool沙盒运行时尚未接通","runtime_ready":false}),
        ),
    }
}
pub(super) async fn status(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    inspect(state, id).await.map(Json)
}
pub(super) async fn prepare(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    if !request.confirm {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "请明确提交准备操作",
        ));
    }
    let lookup = id.clone();
    let actor = principal.id.clone();
    let (kind, prepared_revision) = storage(state.registry.0.clone(), move |store| {
        let record = store.capability(&RegistryAccess::Administrator, &lookup)?;
        if record.revision != request.expected_revision {
            return Err(StoreError::Conflict);
        }
        let kind = record.manifest.kind();
        let mut prepared_revision = record.revision;
        if matches!(kind, CapabilityKind::McpServer | CapabilityKind::Skill) {
            // A preparation request never reverses an explicit deny or grants
            // new permissions. Permissions have their own administrator action.
            let permissions =
                store.capability_permissions(&RegistryAccess::Administrator, &lookup)?;
            if permissions
                .iter()
                .any(|p| p.decision != PermissionDecision::Allow)
            {
                return Err(StoreError::NotAuthorized);
            }
            prepared_revision = store
                .set_capability_enabled(
                    &RegistryAccess::Administrator,
                    &lookup,
                    record.revision,
                    true,
                    &actor,
                )?
                .revision;
        }
        Ok((kind, prepared_revision))
    })
    .await?;
    if kind == CapabilityKind::McpServer {
        state
            .mcp
            .validate(RegistryAccess::Administrator, id.clone(), principal.id)
            .await?;
    } else if kind == CapabilityKind::Skill
        && let Some(runtime) = &state.runtime
    {
        runtime
            .prepare(RegistryAccess::Administrator, id.clone(), prepared_revision)
            .await
            .map_err(super::skill_runtime::error)?;
    }
    inspect(state, id).await.map(Json)
}
