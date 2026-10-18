// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ExternalState, assistant, work};
use crate::{
    errors::{ApiError, storage},
    identity::IdentityError,
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use exoharness_identity::external::Grant;
use exoharness_storage_postgres::ExternalSessionRequest;
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) async fn me(
    State(state): State<ExternalState>,
    Extension(grant): Extension<Grant>,
) -> Result<Json<Value>, IdentityError> {
    let user = grant.user_id.clone();
    let account = work(&state, move |s| s.user_account(&user))
        .await?
        .ok_or_else(IdentityError::not_found)?;
    Ok(Json(
        json!({"account":account,"client_id":grant.client_id,"assistant_id":grant.assistant_id,"scope":grant.scopes.join(" ")}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Create {
    title: Option<String>,
    device_id: Option<String>,
}
pub(super) async fn create(
    State(state): State<ExternalState>,
    Extension(grant): Extension<Grant>,
    headers: HeaderMap,
    Json(body): Json<Create>,
) -> Result<(StatusCode, Json<Value>), IdentityError> {
    if grant.require_device_id && body.device_id.is_none() {
        return Err(IdentityError::new(
            StatusCode::BAD_REQUEST,
            "device_binding_required",
            "This application requires a verified device_id to create a conversation",
        ));
    }
    if grant
        .device_id
        .as_ref()
        .is_some_and(|bound| body.device_id.as_ref() != Some(bound))
    {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "device_mismatch",
            "Conversation device does not match this caller's device",
        ));
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            IdentityError::new(
                StatusCode::BAD_REQUEST,
                "idempotency_key_required",
                "Provide a unique Idempotency-Key for this conversation",
            )
        })?
        .to_owned();
    let profile = assistant(&state, &grant.assistant_id).await?;
    let result = storage(state.api.execution.0.clone(), move |s| {
        s.create_external_session_with_device(
            ExternalSessionRequest {
                owner: &grant.user_id,
                client_id: &grant.client_id,
                request_key: &key,
                title: body.title.as_deref(),
                profile: &profile,
            },
            body.device_id.as_deref(),
        )
    })
    .await?;
    Ok((StatusCode::CREATED, Json(result)))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Device {
    device_id: String,
}
pub(super) async fn device(
    State(state): State<ExternalState>,
    Extension(grant): Extension<Grant>,
    Path(session): Path<String>,
    Json(body): Json<Device>,
) -> Result<Json<Value>, ApiError> {
    if grant
        .device_id
        .as_ref()
        .is_some_and(|device| device != &body.device_id)
    {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "device_mismatch",
            "Device ID does not match the authenticated caller",
        ));
    }
    Ok(Json(
        storage(state.api.execution.0.clone(), move |s| {
            s.set_external_session_device(
                &grant.user_id,
                &grant.client_id,
                &grant.assistant_id,
                &session,
                &body.device_id,
            )
        })
        .await?,
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct History {
    before_ordinal: Option<i64>,
    turn_limit: Option<usize>,
}
pub(super) async fn detail(
    State(state): State<ExternalState>,
    Extension(grant): Extension<Grant>,
    Path(session): Path<String>,
    Query(query): Query<History>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        storage(state.api.execution.0.clone(), move |s| {
            s.conversation_detail(
                &grant.user_id,
                &session,
                query.before_ordinal,
                query.turn_limit.unwrap_or(8),
            )
        })
        .await?,
    ))
}
