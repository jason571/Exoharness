// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::ConfigurationState;
use crate::errors::ApiError;
use axum::{Json, extract::State, http::StatusCode};
use serde_json::Value;

pub(super) async fn chat_configuration(
    State(state): State<ConfigurationState>,
) -> Result<Json<Value>, ApiError> {
    let Json(catalog) = catalog(State(state)).await?;
    let selected = catalog
        .pointer("/default_models/llm")
        .and_then(Value::as_str)
        .or_else(|| catalog["default_model"].as_str());
    let model = selected.and_then(|name| {
        catalog["models"]
            .as_array()?
            .iter()
            .find(|model| model["name"].as_str() == Some(name) && model["enabled"] == true)
    });
    Ok(Json(
        serde_json::json!({"model_display_name":model.and_then(|model|model["display_name"].as_str()),"configuration_managed":true}),
    ))
}

pub(super) async fn catalog(
    State(state): State<ConfigurationState>,
) -> Result<Json<Value>, ApiError> {
    let models = state.models.clone();
    let catalog = tokio::task::spawn_blocking(move || models.refresh())
        .await
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "configuration_unavailable",
                "模型配置读取失败",
            )
        })?
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "credential_unavailable",
                "模型密钥尚未就绪，不回显密钥",
            )
        })?;
    let callable = state
        .runtime
        .as_ref()
        .and_then(|runtime| runtime.speech_models().ok())
        .unwrap_or_default();
    let mut catalog = catalog;
    catalog["model_capabilities"]=serde_json::json!(catalog["models"].as_array().into_iter().flatten()
        .filter(|model|model["model_type"].as_str().unwrap_or("llm")!="llm")
        .map(|model|serde_json::json!({"name":model["name"],"model_type":model["model_type"],"agent_callable":callable.iter().any(|m|model["name"]==m.name)})).collect::<Vec<_>>());
    Ok(Json(catalog))
}
