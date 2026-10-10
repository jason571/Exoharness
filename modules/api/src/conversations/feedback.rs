// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ConversationState, sessions::History};
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde_json::{Value, json};

pub(super) async fn session(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(session): Path<String>,
    Query(query): Query<History>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        storage(state.store, move |store| {
            store.conversation_feedback(
                &principal.id,
                &session,
                query.before_ordinal,
                query.turn_limit.unwrap_or(30),
            )
        })
        .await?,
    ))
}
pub(super) async fn run(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(run): Path<String>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        storage(state.store, move |store| {
            store.response_feedback(&principal.id, &run)
        })
        .await?,
    ))
}
pub(super) async fn record(
    State(state): State<ConversationState>,
    Extension(principal): Extension<Principal>,
    Path(run): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    if body["action"] == "regenerate" {
        return Err(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "agent_not_ready",
            "执行模块尚未接通，本次未创建重新生成任务",
        ));
    }
    let state = storage(state.store, move |store| {
        store.record_response_feedback(&principal.id, &run, &body)
    })
    .await?;
    Ok(Json(json!({"state":state})))
}
