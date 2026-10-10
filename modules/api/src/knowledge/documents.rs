use super::{KnowledgeState, Params, id, new_id, scope};
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) async fn list(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    Ok(Json(json!(
        storage(state.store, move |store| store
            .knowledge_documents(&owner, None, None))
        .await?
    )))
}
pub(super) async fn detail(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let id = id(&params)?.to_owned();
    let values = storage(state.store, move |store| {
        store.knowledge_documents(&owner, Some(&id), None)
    })
    .await?;
    Ok(Json(
        values
            .into_iter()
            .next()
            .expect("store requires matching document"),
    ))
}
pub(super) async fn create(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let id = new_id("document")?;
    Ok((
        StatusCode::CREATED,
        Json(
            storage(state.store, move |store| {
                store.save_knowledge_document(&owner, &id, true, &body)
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
            store.save_knowledge_document(&owner, &id, false, &body)
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
        store.delete_knowledge_document(&owner, &id)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
pub(super) struct Search {
    q: String,
}
pub(super) async fn search(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Query(query): Query<Search>,
) -> Result<Json<Value>, ApiError> {
    if query.q.trim().is_empty() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "empty_search",
            "请输入检索内容",
        ));
    }
    Ok(Json(json!(
        storage(state.store, move |store| store.knowledge_documents(
            &principal.id,
            None,
            Some(&query.q)
        ))
        .await?
    )))
}
