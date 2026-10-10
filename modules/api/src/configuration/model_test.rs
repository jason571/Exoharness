// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Admin-confirmed diagnostic request, never a fake test or implicit generation.
use super::{
    ConfigurationState,
    model_requests::{self, Test},
};
use crate::errors::ApiError;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_model_runtime::{CancellationToken, ErrorKind, ModelError};
use serde_json::Value;

pub(super) async fn test(
    State(state): State<ConfigurationState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Value>, ApiError> {
    let request: Test = model_requests::parse(body)?;
    let _provenance = request.actor;
    if !request.confirm {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "测试模型需要明确确认，将发送一次诊断请求",
        ));
    }
    let runtime = state.runtime.ok_or(ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "model_adapter_not_ready",
        "模型调用适配器尚未接通，未发送测试请求",
    ))?;
    let cancel = CancellationToken::new();
    let _on_drop = cancel.clone().drop_guard();
    let output = runtime.test(&name, cancel).await.map_err(error)?;
    serde_json::to_value(output).map(Json).map_err(|_| {
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            "diagnostic_encoding_failed",
            "测试结果序列化失败",
        )
    })
}
fn error(error: ModelError) -> ApiError {
    match error.kind {
        ErrorKind::Configuration => ApiError(
            StatusCode::CONFLICT,
            "model_configuration_unavailable",
            "模型未启用或配置、密钥不可用，未确认测试成功",
        ),
        ErrorKind::Unsupported => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_protocol_not_ready",
            "该类型或参数的调用适配器尚未接通，未发送测试请求",
        ),
        ErrorKind::Busy => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "model_busy",
            "模型测试队列已满，请稍后重试",
        ),
        ErrorKind::Timeout => ApiError(
            StatusCode::GATEWAY_TIMEOUT,
            "model_timeout",
            "模型测试请求超时，未自动重试",
        ),
        ErrorKind::Cancelled => ApiError(
            StatusCode::REQUEST_TIMEOUT,
            "model_cancelled",
            "模型测试已取消",
        ),
        ErrorKind::Http if error.status == Some(429) => ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "model_rate_limited",
            "上游模型限流，本次未自动重试",
        ),
        ErrorKind::Http if matches!(error.status, Some(401 | 403)) => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_authentication_failed",
            "模型服务认证失败，请检查密钥及账户/模型访问权限；本次未自动重试",
        ),
        ErrorKind::Http if error.status == Some(404) => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_endpoint_not_found",
            "模型接口不存在，请检查所选协议和服务地址；本次未自动重试",
        ),
        ErrorKind::Http if error.status == Some(400) => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_request_rejected",
            "模型服务拒绝请求参数，请检查模型ID和采样/音色配置；本次未自动重试",
        ),
        ErrorKind::Http => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_http_error",
            "上游模型返回HTTP错误，请检查密钥和接口配置；本次未自动重试",
        ),
        ErrorKind::ProviderRejected => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_provider_rejected",
            "模型服务拒绝了测试，请检查模型、音色和服务权限；本次未自动重试",
        ),
        ErrorKind::InvalidRequest => ApiError(
            StatusCode::BAD_REQUEST,
            "model_request_invalid",
            "请求参数不符合所选模型协议，未发送测试请求",
        ),
        ErrorKind::OutputLimit => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_output_limit",
            "模型响应达到输出上限，不能视为测试成功",
        ),
        ErrorKind::Incomplete => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_response_incomplete",
            "模型响应未完整结束，不能视为测试成功",
        ),
        ErrorKind::Transport => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_connection_failed",
            "无法连接模型服务，请检查服务地址、网络和TLS证书；本次未自动重试",
        ),
        _ => ApiError(
            StatusCode::BAD_GATEWAY,
            "model_response_invalid",
            "模型响应未通过完整性校验，不能视为测试成功",
        ),
    }
}
