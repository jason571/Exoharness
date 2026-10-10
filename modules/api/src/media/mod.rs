//! Signed read-only delivery server; no cookies, shell or arbitrary URL proxy.
mod files;
mod metadata;
mod project;
mod publishing;
mod range;
mod stream;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use exoharness_artifacts::Artifacts;
use serde_json::json;
use std::sync::Arc;
#[derive(Clone)]
struct MediaState {
    artifacts: Arc<Artifacts>,
    port: u16,
    io: Arc<tokio::sync::Semaphore>,
}
pub fn media_router(artifacts: Arc<Artifacts>, port: u16) -> Router {
    let state = MediaState {
        artifacts,
        port,
        io: Arc::new(tokio::sync::Semaphore::new(8)),
    };
    Router::new()
        .route("/health", get(health))
        .route("/projects/{id}/{signature}/{*path}", get(project::get))
        .route("/artifacts/{id}/{filename}", get(files::download))
        .route("/artifacts/{id}/{filename}/metadata", get(metadata::get))
        .route("/artifacts/{id}/{filename}/preview", get(files::preview))
        .route(
            "/workspace-files/{id}/{filename}",
            get(files::workspace_download),
        )
        .route(
            "/workspace-files/{id}/{filename}/preview",
            get(files::preview),
        )
        .fallback(|| async {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error":{"code":"file_unavailable","message":"文件不存在或链接无效"}})),
            )
        })
        .layer(middleware::from_fn_with_state(state.clone(), host))
        .with_state(state)
}
async fn host(State(state): State<MediaState>, request: Request, next: Next) -> Response {
    let local = format!("127.0.0.1:{}", state.port);
    let localhost = format!("localhost:{}", state.port);
    let public = state.artifacts.public_host();
    if !request
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| host == local || host == localhost || host == public)
    {
        return crate::errors::ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_host",
            "媒体服务地址不匹配",
        )
        .into_response();
    }
    next.run(request).await
}
use axum::response::IntoResponse;
async fn health(
    State(state): State<MediaState>,
) -> Result<Json<serde_json::Value>, crate::errors::ApiError> {
    let permit = state.io.try_acquire_owned().map_err(|_| {
        crate::errors::ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_busy",
            "文件服务暂忙",
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        state.artifacts.health()
    })
    .await
    .map_err(|_| {
        crate::errors::ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_unavailable",
            "文件服务暂不可用",
        )
    })?
    .map_err(|_| {
        crate::errors::ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_unavailable",
            "文件服务暂不可用",
        )
    })?;
    Ok(Json(
        json!({"service":"exoharness-media","status":"ok","delivery_only":true}),
    ))
}
pub use publishing::artifact_management_router;
