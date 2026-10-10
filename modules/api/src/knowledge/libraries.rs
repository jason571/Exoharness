// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{KnowledgeState, Params, new_id};
use crate::errors::{ApiError, storage};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};

fn library(params: &Params) -> String {
    params["library"].clone()
}
pub(super) async fn list(State(state): State<KnowledgeState>) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        storage(state.store, move |store| store.knowledge_libraries(None)).await?
    )))
}
pub(super) async fn detail(
    State(state): State<KnowledgeState>,
    Path(params): Path<Params>,
) -> Result<Json<Value>, ApiError> {
    let id = library(&params);
    let values = storage(state.store, move |store| {
        store.knowledge_libraries(Some(&id))
    })
    .await?;
    Ok(Json(
        values.into_iter().next().expect("store requires library"),
    ))
}
pub(super) async fn create(
    State(state): State<KnowledgeState>,
    Json(body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let id = new_id("library")?;
    Ok((
        StatusCode::CREATED,
        Json(
            storage(state.store, move |store| {
                store.save_knowledge_library(&id, true, &body)
            })
            .await?,
        ),
    ))
}
pub(super) async fn update(
    State(state): State<KnowledgeState>,
    Path(params): Path<Params>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let id = library(&params);
    Ok(Json(
        storage(state.store, move |store| {
            store.save_knowledge_library(&id, false, &body)
        })
        .await?,
    ))
}
pub(super) async fn delete(
    State(state): State<KnowledgeState>,
    Path(params): Path<Params>,
) -> Result<StatusCode, ApiError> {
    let id = library(&params);
    storage(state.store, move |store| {
        store.delete_knowledge_library(&id)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Grants {
    assistant_ids: Vec<String>,
}
pub(super) async fn grants(
    State(state): State<KnowledgeState>,
    Path(params): Path<Params>,
    Json(body): Json<Grants>,
) -> Result<Json<Value>, ApiError> {
    if body.assistant_ids.len() > 100 {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_grants",
            "授权助理数量超出限制",
        ));
    }
    let assistants = state.assistants.clone();
    let ids = body.assistant_ids.clone();
    let valid = tokio::task::spawn_blocking(move || {
        assistants.refresh()?;
        let profiles = assistants.list();
        Ok::<_, exoharness_configuration::ConfigurationError>(ids.iter().all(|id| {
            profiles
                .iter()
                .any(|profile| profile["id"].as_str() == Some(id) && profile["enabled"] == true)
        }))
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "assistant_worker_failed",
            "助理配置读取中断",
        )
    })?
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "assistant_unavailable",
            "无法核对助理配置",
        )
    })?;
    if !valid {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_assistant",
            "只能授予已登记并启用的助理",
        ));
    }
    let id = library(&params);
    Ok(Json(
        storage(state.store, move |store| {
            store.grant_knowledge_library(&id, &body.assistant_ids)
        })
        .await?,
    ))
}
