// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::RegistryState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use exoharness_contracts::capability::PermissionDecision;
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Enabled {
    enabled: bool,
    expected_revision: i64,
    #[serde(default)]
    actor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Decision {
    decision: PermissionDecision,
    #[serde(default)]
    expected_revision: Option<i64>,
    #[serde(default)]
    actor: Option<String>,
}
pub(super) async fn enabled(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Json(request): Json<Enabled>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    storage(state.registry.0.clone(), move |s| {
        Ok(json!(s.set_capability_enabled(
            &RegistryAccess::Administrator,
            &id,
            request.expected_revision,
            request.enabled,
            &principal.id
        )?))
    })
    .await
    .map(Json)
}
pub(super) async fn permission(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path((id, permission)): Path<(String, String)>,
    Json(request): Json<Decision>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    storage(state.registry.0.clone(), move |s| {
        let expected = match request.expected_revision {
            Some(value) => value,
            None => s.capability(&RegistryAccess::Administrator, &id)?.revision,
        };
        Ok(json!(s.decide_capability_permission(
            &RegistryAccess::Administrator,
            &id,
            expected,
            &permission,
            request.decision,
            &principal.id
        )?))
    })
    .await
    .map(Json)
}
