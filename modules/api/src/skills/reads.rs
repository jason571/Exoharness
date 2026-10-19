// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{SkillState, errors::SkillHttpError, work};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Revision {
    revision: i64,
}

/// Administrator-only immutable export. Revision CAS binds metadata and files
/// to one package snapshot, without executing scripts or reading host paths.
pub(super) async fn package(
    State(state): State<SkillState>,
    Extension(principal): Extension<crate::auth::Principal>,
    Path(id): Path<String>,
    Query(query): Query<Revision>,
) -> Result<Json<Value>, Response> {
    if principal.role != crate::auth::Role::Admin {
        return Err(crate::errors::ApiError(
            StatusCode::FORBIDDEN,
            "management_required",
            "Administrator required",
        )
        .into_response());
    }
    work(state, move |state| {
        let snapshot =
            state
                .skills
                .package_snapshot(&RegistryAccess::Administrator, &id, query.revision)?;
        let files: Vec<_> = snapshot
            .files
            .into_iter()
            .map(|(path, bytes)| json!({"path":path,"content_base64":STANDARD.encode(bytes)}))
            .collect();
        Ok(json!({"capability":snapshot.capability,"files":files}))
    })
    .await
    .map(Json)
    .map_err(IntoResponse::into_response)
}

pub(super) async fn instructions(
    State(state): State<SkillState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, SkillHttpError> {
    work(state, move |state| {
        let (record, content) = state.skills.instruction_view(&RegistryAccess::Administrator, &id)?;
        Ok(json!({"content":content,"metadata":{"capability_id":record.id,"name":record.manifest.name,
            "description":record.manifest.description,"version":record.manifest.version},"revision":record.revision,"source_digest":record.source_digest}))
    }).await.map(Json)
}
pub(super) async fn files(
    State(state): State<SkillState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, SkillHttpError> {
    work(state, move |state| {
        Ok(json!({"files":state.skills.files(&RegistryAccess::Administrator, &id)?}))
    })
    .await
    .map(Json)
}
