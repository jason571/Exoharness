// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::ConfigurationState;
use crate::{auth, errors::ApiError};
use axum::{
    Extension, Json,
    extract::{Path, State},
};
use serde_json::Value;

pub(super) async fn assistant_list(
    State(state): State<ConfigurationState>,
) -> Result<Json<Vec<Value>>, ApiError> {
    super::assistant_writes::work(state, |s| {
        s.assistants.refresh()?;
        Ok(s.assistants.list())
    })
    .await
    .map(Json)
}
pub(super) async fn assistant_detail(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    super::assistant_writes::work(state, move |s| {
        s.assistants.refresh()?;
        super::assistant_authorization::current(&s.assistants, &id)
    })
    .await
    .map(Json)
}
pub(super) async fn user_profile(
    state: &ConfigurationState,
    principal: &auth::Principal,
) -> Result<Value, ApiError> {
    super::assistant_writes::work(state.clone(), |s| s.assistants.refresh()).await?;
    if !state.management
        && let Some(identity) = state.identity.clone()
    {
        let id = principal.id.clone();
        let assigned =
            crate::identity::work(identity, move |store| store.user_agent_profile_id(&id)).await?;
        if let Some(id) = assigned {
            let profile = state.assistants.public_profile(&id);
            if !profile.is_null() {
                return Ok(profile);
            }
        }
    }
    Ok(state.assistants.default_public())
}
pub(super) async fn default_assistant(
    State(state): State<ConfigurationState>,
    Extension(principal): Extension<auth::Principal>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(user_profile(&state, &principal).await?))
}
