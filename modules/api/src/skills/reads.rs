// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{SkillState, errors::SkillHttpError, work};
use axum::{
    Json,
    extract::{Path, State},
};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

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
