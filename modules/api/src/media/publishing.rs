//! Authenticated diagnostic ingestion. Agent publishing uses Artifacts directly.
use crate::{
    ApiState, Surface,
    auth::{self, Principal, Role},
    errors::ApiError,
};
use axum::{
    Extension, Json, Router,
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    middleware,
    routing::post,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_artifacts::Artifacts;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{io::Write, sync::Arc};
#[derive(Clone)]
struct PublishState {
    artifacts: Arc<Artifacts>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    filename: String,
    content_base64: String,
}
pub fn artifact_management_router(api: ApiState, artifacts: Arc<Artifacts>) -> Router {
    if api.surface != Surface::Management {
        return Router::new();
    }
    Router::new()
        .route(
            "/v1/local/sessions/{session}/runs/{run}/artifacts",
            post(publish),
        )
        .layer(DefaultBodyLimit::max(2 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .with_state(PublishState { artifacts })
}
async fn publish(
    State(state): State<PublishState>,
    Extension(principal): Extension<Principal>,
    Path((session, run)): Path<(String, String)>,
    Json(input): Json<Input>,
) -> Result<Json<Value>, ApiError> {
    if principal.role != Role::Admin {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "not_authorized",
            "文件登记验证仅限管理员",
        ));
    }
    let bytes = STANDARD
        .decode(input.content_base64)
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "invalid_file", "文件编码不正确"))?;
    if bytes.len() > 1024 * 1024 {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "diagnostic_file_too_large",
            "诊断导入仅支持1 MiB；正式交付使用流式文件接口",
        ));
    }
    let saved = tokio::task::spawn_blocking(move || {
        let mut file =
            tempfile::tempfile().map_err(|_| exoharness_artifacts::ArtifactError::Storage)?;
        file.write_all(&bytes)
            .map_err(|_| exoharness_artifacts::ArtifactError::Storage)?;
        state
            .artifacts
            .publish_file(&principal.id, &session, &run, &input.filename, file)
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_unavailable",
            "文件服务暂不可用",
        )
    })?
    .map_err(|error| match error {
        exoharness_artifacts::ArtifactError::Store(error) => error.into(),
        _ => ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_file",
            "文件交付登记未通过",
        ),
    })?;
    Ok(Json(json!(saved)))
}
