// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{SkillState, errors::SkillHttpError, requests, work};
use crate::auth::Principal;
use axum::{Extension, Json, body::Bytes, extract::State};
use exoharness_skills::Preview;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

pub(super) async fn preview(
    State(state): State<SkillState>,
    bytes: Bytes,
) -> Result<Json<Preview>, SkillHttpError> {
    work(state, move |state| {
        state
            .skills
            .preview(&RegistryAccess::Administrator, &requests::upload(&bytes)?)
    })
    .await
    .map(Json)
}
pub(super) async fn import(
    State(state): State<SkillState>,
    Extension(principal): Extension<Principal>,
    bytes: Bytes,
) -> Result<Json<Value>, SkillHttpError> {
    let record = work(state, move |state| {
        state.skills.import(
            &RegistryAccess::Administrator,
            &requests::upload(&bytes)?,
            &principal.id,
        )
    })
    .await?;
    Ok(Json(
        json!({"capabilities":[record], "warnings":["Skill 内容已保存和登记；未执行任何脚本。"]}),
    ))
}
