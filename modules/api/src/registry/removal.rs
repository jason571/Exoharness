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
use exoharness_contracts::capability::CapabilityKind;
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::{Value, json};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    expected_revision: i64,
    #[serde(default)]
    actor: Option<String>,
}
pub(super) async fn remove(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    let lookup = id.clone();
    let record = storage(state.registry.0.clone(), move |store| {
        store.capability(&RegistryAccess::Administrator, &lookup)
    })
    .await?;
    if record.manifest.kind() == CapabilityKind::Skill {
        let skills = state.skills;
        tokio::task::spawn_blocking(move || {
            skills.remove(
                &RegistryAccess::Administrator,
                &id,
                request.expected_revision,
                &principal.id,
            )
        })
        .await
        .map_err(|_| exoharness_mcp::McpError::Configuration)?
        .map_err(ApiError::from)?;
        return Ok(Json(json!({"deleted":true,"files_removed":true})));
    }
    // Secret versions can be referenced by fixed execution snapshots. Retire
    // the registry identity, never eagerly destroy immutable credential files.
    storage(state.registry.0.clone(), move |store| {
        store.remove_capability(
            &RegistryAccess::Administrator,
            &id,
            request.expected_revision,
            &principal.id,
        )
    })
    .await?;
    Ok(Json(json!({"deleted":true,"files_removed":false})))
}
