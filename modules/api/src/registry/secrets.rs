// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::RegistryState;
use crate::{auth::Principal, errors::ApiError};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Update {
    value: String,
    #[serde(default)]
    expected_revision: Option<i64>,
    #[serde(default)]
    actor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Clear {
    #[serde(default)]
    expected_revision: Option<i64>,
    #[serde(default)]
    actor: Option<String>,
}
pub(super) async fn update(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path((id, name)): Path<(String, String)>,
    Json(request): Json<Update>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    let record = crate::mcp::work(state.mcp, move |mcp| {
        mcp.update_secret(
            &RegistryAccess::Administrator,
            &id,
            &name,
            Some(&request.value),
            request.expected_revision,
            &principal.id,
        )
    })
    .await?;
    Ok(Json(json!({"configured":true,"revision":record.revision})))
}
pub(super) async fn clear(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path((id, name)): Path<(String, String)>,
    Json(request): Json<Clear>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    let record = crate::mcp::work(state.mcp, move |mcp| {
        mcp.update_secret(
            &RegistryAccess::Administrator,
            &id,
            &name,
            None,
            request.expected_revision,
            &principal.id,
        )
    })
    .await?;
    Ok(Json(json!({"configured":false,"revision":record.revision})))
}
