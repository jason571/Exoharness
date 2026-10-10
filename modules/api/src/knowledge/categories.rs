// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{KnowledgeState, Params, id, new_id, scope};
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde_json::{Value, json};

pub(super) async fn list(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    Ok(Json(json!(
        storage(state.store, move |store| store.knowledge_categories(&owner)).await?
    )))
}
pub(super) async fn create(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let id = new_id("category")?;
    Ok((
        StatusCode::CREATED,
        Json(
            storage(state.store, move |store| {
                store.save_knowledge_category(&owner, &id, true, &body)
            })
            .await?,
        ),
    ))
}
pub(super) async fn update(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let id = id(&params)?.to_owned();
    Ok(Json(
        storage(state.store, move |store| {
            store.save_knowledge_category(&owner, &id, false, &body)
        })
        .await?,
    ))
}
pub(super) async fn delete(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<StatusCode, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let id = id(&params)?.to_owned();
    storage(state.store, move |store| {
        store.delete_knowledge_category(&owner, &id)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
