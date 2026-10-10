// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Original management contract, independent of model execution adapters.
use super::{
    ConfigurationState,
    model_requests::{self, Action, Configuration, Enabled, Model, Provider},
};
use crate::errors::ApiError;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_configuration::{ConfigurationError, Models};
use serde_json::Value;

fn error(value: ConfigurationError) -> ApiError {
    match value {
        ConfigurationError::Conflict => ApiError(
            StatusCode::CONFLICT,
            "configuration_conflict",
            "配置已变化或仍被模型引用，请刷新后核对",
        ),
        ConfigurationError::NotFound => ApiError(
            StatusCode::NOT_FOUND,
            "configuration_not_found",
            "未找到对应模型配置",
        ),
        ConfigurationError::PrivateFile => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_store_unavailable",
            "配置存储暂时不可用，尚未确认保存；请刷新核对",
        ),
        ConfigurationError::Json => ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_configuration_json",
            "配置数据不是有效的 JSON",
        ),
        ConfigurationError::Invalid(message) => {
            ApiError(StatusCode::BAD_REQUEST, "invalid_configuration", message)
        }
    }
}
pub(super) async fn work<T, F>(state: ConfigurationState, action: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&Models) -> Result<T, ConfigurationError> + Send + 'static,
{
    let permit = state.workers.acquire_owned().await.map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_busy",
            "配置工作服务已关闭",
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        action(&state.models)
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_worker_failed",
            "配置处理已中断，请刷新核对是否保存",
        )
    })?
    .map_err(error)
}
pub(super) async fn configuration(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request: Configuration = model_requests::parse(body)?;
    let _provenance = request.actor;
    let value = work(state, move |s| {
        s.save_configuration(
            request.provider,
            request.model,
            request.api_key.as_deref(),
            request.expected_revision,
        )
    })
    .await?;
    Ok((StatusCode::CREATED, Json(value)))
}
pub(super) async fn provider(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request: Provider = model_requests::parse(body)?;
    let _provenance = request.actor;
    Ok((
        StatusCode::CREATED,
        Json(
            work(state, move |s| {
                s.save_provider(request.provider, request.expected_revision)
            })
            .await?,
        ),
    ))
}
pub(super) async fn model(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request: Model = model_requests::parse(body)?;
    let _provenance = request.actor;
    Ok((
        StatusCode::CREATED,
        Json(
            work(state, move |s| {
                s.save_model(request.model, request.expected_revision)
            })
            .await?,
        ),
    ))
}
pub(super) async fn model_enabled(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Enabled = model_requests::parse(body)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.set_model_enabled(&name, request.enabled, request.expected_revision)
    })
    .await
    .map(Json)
}
pub(super) async fn provider_enabled(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Enabled = model_requests::parse(body)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.set_provider_enabled(&name, request.enabled, request.expected_revision)
    })
    .await
    .map(Json)
}
pub(super) async fn default_model(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Action = model_requests::parse(body)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.set_default(&name, request.expected_revision)
    })
    .await
    .map(Json)
}
pub(super) async fn remove_model(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let request: Action = model_requests::parse(body)?;
    let _provenance = request.actor;
    confirmed(request.confirm)?;
    work(state, move |s| {
        s.remove_model(&name, request.expected_revision)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn remove_provider(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let request: Action = model_requests::parse(body)?;
    let _provenance = request.actor;
    confirmed(request.confirm)?;
    work(state, move |s| {
        s.remove_provider(&name, request.expected_revision)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
fn confirmed(confirm: bool) -> Result<(), ApiError> {
    if confirm {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "删除配置需明确确认",
        ))
    }
}
