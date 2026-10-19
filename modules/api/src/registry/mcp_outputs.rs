// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Administrator-only, revision-checked output-boundary configuration.
use super::RegistryState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use exoharness_contracts::capability::{CapabilitySpec, ValidatedCapability};
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    expected_revision: i64,
    artifact_origins: Vec<String>,
}

pub(super) async fn update(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |store| {
        let before = store.capability(&RegistryAccess::Administrator, &id)?;
        if before.revision != request.expected_revision {
            return Err(StoreError::Conflict);
        }
        let mut manifest = before.manifest.clone();
        let CapabilitySpec::McpServer(spec) = &mut manifest.spec else {
            return Err(StoreError::Invalid("Output origins require an MCP service"));
        };
        if spec.artifact_origins == request.artifact_origins {
            return Ok(json!(before));
        }
        spec.artifact_origins = request.artifact_origins;
        let checked = ValidatedCapability::new(manifest, before.source_digest).map_err(|_| {
            StoreError::Invalid("MCP output origins must be valid unique HTTP(S) origins")
        })?;
        store
            .update_capability(
                &RegistryAccess::Administrator,
                &id,
                checked,
                request.expected_revision,
                &principal.id,
            )
            .map(|record| json!(record))
    })
    .await
    .map(Json)
}
