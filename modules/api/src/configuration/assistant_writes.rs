// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Original assistant write contracts, with blocking file/DB work off the reactor.
use super::{
    ConfigurationState, assistant_authorization,
    assistant_requests::{self, Action, Avatar, Enabled, Write},
};
use crate::errors::ApiError;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use exoharness_configuration::ConfigurationError;
use serde_json::Value;

pub(super) fn error(error: ConfigurationError) -> ApiError {
    match error {
        ConfigurationError::Conflict => ApiError(
            StatusCode::CONFLICT,
            "assistant_conflict",
            "助理已更新，或仍是默认助理/被用户使用；请刷新核对",
        ),
        ConfigurationError::NotFound => {
            ApiError(StatusCode::NOT_FOUND, "assistant_not_found", "未找到该助理")
        }
        ConfigurationError::PrivateFile => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "assistant_store_unavailable",
            "助理存储暂时不可用，尚未确认保存；请刷新核对",
        ),
        ConfigurationError::Json => ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_assistant_json",
            "助理配置不是有效的 JSON",
        ),
        ConfigurationError::Invalid(message) => ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_assistant_configuration",
            message,
        ),
    }
}
pub(super) async fn work<T, F>(state: ConfigurationState, action: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce(&ConfigurationState) -> Result<T, ConfigurationError> + Send + 'static,
{
    let permit = state.workers.clone().acquire_owned().await.map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration_busy",
            "配置服务已关闭",
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        action(&state)
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "assistant_worker_failed",
            "助理处理已中断，请刷新核对是否保存",
        )
    })?
    .map_err(error)
}
pub(super) async fn create(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request: Write = assistant_requests::parse(body)?;
    let input = request.input();
    let _provenance = request.actor;
    let id = request.id.ok_or(ApiError(
        StatusCode::BAD_REQUEST,
        "assistant_id_required",
        "创建助理需要 ID",
    ))?;
    if request.expected_revision.is_some() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_create",
            "创建助理不能指定已有版本",
        ));
    }
    let value = work(state, move |s| {
        assistant_authorization::validate(s, None, &input)?;
        s.assistants.create(
            &id,
            input,
            request.enabled.unwrap_or(true),
            request.make_default.unwrap_or(false),
        )?;
        assistant_authorization::current(&s.assistants, &id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(value)))
}
pub(super) async fn update(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Write = assistant_requests::parse(body)?;
    let input = request.input();
    let _provenance = request.actor;
    if request.id.is_some() || request.enabled.is_some() || request.make_default.is_some() {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_update",
            "更新配置不能创建新的助理或隐式改变启用状态",
        ));
    }
    let expected = request.expected_revision.ok_or(ApiError(
        StatusCode::BAD_REQUEST,
        "revision_required",
        "请刷新助理配置后保存",
    ))?;
    work(state, move |s| {
        assistant_authorization::validate(s, Some(&id), &input)?;
        s.assistants.update(&id, expected, input)?;
        assistant_authorization::current(&s.assistants, &id)
    })
    .await
    .map(Json)
}
pub(super) async fn enabled(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Enabled = assistant_requests::parse(body)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.assistants.set_enabled(&id, request.enabled)?;
        assistant_authorization::current(&s.assistants, &id)
    })
    .await
    .map(Json)
}
pub(super) async fn default(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Action = assistant_requests::parse(body)?;
    confirm(request.confirm)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.assistants.set_default(&id)?;
        assistant_authorization::current(&s.assistants, &id)
    })
    .await
    .map(Json)
}
pub(super) async fn remove(
    State(state): State<ConfigurationState>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    let request: Action = assistant_requests::parse(body)?;
    confirm(request.confirm)?;
    let _provenance = request.actor;
    work(state, move |s| {
        s.assistants
            .remove(&id, || assistant_authorization::referenced(s, &id))
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
fn confirm(value: bool) -> Result<(), ApiError> {
    if value {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "此操作需要明确确认",
        ))
    }
}
pub(super) async fn upload(
    State(state): State<ConfigurationState>,
    body: Bytes,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let request: Avatar = assistant_requests::parse(body)?;
    let _provenance = request.actor;
    let bytes = STANDARD.decode(request.content_base64).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_avatar_encoding",
            "头像数据无效，请重新选择图片",
        )
    })?;
    let value = work(state, move |s| {
        s.assistants.upload_avatar(&request.content_type, &bytes)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(value)))
}
