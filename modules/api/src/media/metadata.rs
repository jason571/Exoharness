//! Signed, read-only artifact identity and hashed execution evidence.
use super::MediaState;
use crate::errors::ApiError;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::Value;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Access {
    #[serde(default)]
    expires: u64,
    signature: String,
}
pub(super) async fn get(
    State(state): State<MediaState>,
    Path((id, name)): Path<(String, String)>,
    Query(access): Query<Access>,
) -> Result<Json<Value>, ApiError> {
    let permit = state.io.try_acquire_owned().map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_busy",
            "文件服务暂忙",
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        state
            .artifacts
            .metadata_signed(&id, &name, access.expires, &access.signature)
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_unavailable",
            "文件服务暂不可用",
        )
    })?
    .map(Json)
    .map_err(|_| {
        ApiError(
            StatusCode::NOT_FOUND,
            "file_unavailable",
            "文件不存在或链接无效",
        )
    })
}
