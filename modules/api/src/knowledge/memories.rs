use super::{KnowledgeState, Params, id, new_id};
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
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        storage(state.store, move |store| store.memories(&principal.id)).await?
    )))
}
pub(super) async fn create(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let id = new_id("memory")?;
    Ok((
        StatusCode::CREATED,
        Json(
            storage(state.store, move |store| {
                store.save_memory(&principal.id, &id, true, &body)
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
    let id = id(&params)?.to_owned();
    Ok(Json(
        storage(state.store, move |store| {
            store.save_memory(&principal.id, &id, false, &body)
        })
        .await?,
    ))
}
pub(super) async fn delete(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<StatusCode, ApiError> {
    let id = id(&params)?.to_owned();
    storage(state.store, move |store| {
        store.delete_memory(&principal.id, &id)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
