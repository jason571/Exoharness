// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Authenticated browser recording adapter. No audio persistence or chat calls.
use super::{ConfigurationState, model_writes};
use crate::errors::ApiError;
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use exoharness_model_runtime::{CancellationToken, ErrorKind, MAX_AUDIO_BYTES};
use serde_json::{Value, json};

async fn selection(state: ConfigurationState) -> Result<Option<String>, ApiError> {
    model_writes::work(state, |models| {
        models.refresh()?;
        Ok(models
            .catalog()
            .pointer("/default_models/asr")
            .and_then(Value::as_str)
            .map(str::to_owned))
    })
    .await
}
pub(super) async fn configuration(
    State(state): State<ConfigurationState>,
) -> Result<Json<Value>, ApiError> {
    let name = selection(state.clone()).await?;
    let enabled = name.is_some();
    let cancel = CancellationToken::new();
    let _on_drop = cancel.clone().drop_guard();
    let available = match (state.runtime, name) {
        (Some(runtime), Some(name)) => runtime.transcription_available(&name, cancel).await.is_ok(),
        _ => false,
    };
    Ok(Json(
        json!({"enabled":enabled,"available":available,"max_recording_seconds":120,"max_audio_bytes":MAX_AUDIO_BYTES,"message":if available {"语音输入可用，点击开始录音"} else if enabled {"语音服务暂不可用，请稍后重试"} else {"尚未配置语音输入，请联系管理员"}}),
    ))
}
pub(super) async fn transcribe(
    State(state): State<ConfigurationState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let name = selection(state.clone()).await?.ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "speech_not_configured",
        "尚未配置语音输入，请联系管理员",
    ))?;
    let runtime = state.runtime.ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "speech_not_ready",
        "语音输入尚未接通",
    ))?;
    let mime = headers
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    let cancel = CancellationToken::new();
    let _on_drop = cancel.clone().drop_guard();
    let text = runtime
        .transcribe(&name, body.to_vec(), mime, cancel)
        .await
        .map_err(|error| match error.kind {
            ErrorKind::InvalidRequest => ApiError(
                StatusCode::BAD_REQUEST,
                "invalid_recording",
                "录音为空、过大或格式不受支持，请重新录制",
            ),
            ErrorKind::Busy => ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "speech_busy",
                "语音服务繁忙，请稍后重试",
            ),
            ErrorKind::Http if error.status == Some(429) => ApiError(
                StatusCode::TOO_MANY_REQUESTS,
                "speech_busy",
                "语音服务繁忙，请稍后重试",
            ),
            ErrorKind::Timeout => ApiError(
                StatusCode::GATEWAY_TIMEOUT,
                "speech_timeout",
                "语音识别超时，请重新录制",
            ),
            _ => ApiError(
                StatusCode::BAD_GATEWAY,
                "speech_unavailable",
                "语音识别暂不可用，请稍后重试；仍可直接输入文字",
            ),
        })?;
    Ok(Json(json!({"text":text})))
}
