// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::ConfigurationState;
use crate::{auth, errors::ApiError};
use axum::{
    Extension,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::Value;

pub(super) async fn avatar(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
    Extension(principal): Extension<auth::Principal>,
) -> Result<Response, ApiError> {
    if !state.management
        && super::profiles::user_profile(&state, &principal)
            .await?
            .pointer("/avatar/asset_id")
            .and_then(Value::as_str)
            != Some(id.as_str())
    {
        return Err(ApiError(StatusCode::NOT_FOUND, "not_found", "未找到该头像"));
    }
    let (kind, bytes) = super::assistant_writes::work(state, move |s| {
        s.assistants
            .avatar(&id)
            .ok_or(exoharness_configuration::ConfigurationError::NotFound)
    })
    .await?;
    Ok((
        [
            (header::CONTENT_TYPE, kind),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        ],
        bytes,
    )
        .into_response())
}
