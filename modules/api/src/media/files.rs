// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{MediaState, range};
use crate::errors::ApiError;
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, Method, StatusCode},
    response::Response,
};
use exoharness_artifacts::ArtifactError;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Access {
    #[serde(default)]
    expires: u64,
    signature: String,
    #[serde(default)]
    download: bool,
}
pub(super) async fn download(
    State(state): State<MediaState>,
    Path(path): Path<(String, String)>,
    Query(access): Query<Access>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    deliver(state, path, access, method, headers, false, false).await
}
pub(super) async fn workspace_download(
    State(state): State<MediaState>,
    Path(path): Path<(String, String)>,
    Query(access): Query<Access>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    deliver(state, path, access, method, headers, false, true).await
}
pub(super) async fn preview(
    State(state): State<MediaState>,
    Path(path): Path<(String, String)>,
    Query(access): Query<Access>,
    method: Method,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    deliver(state, path, access, method, headers, true, false).await
}
fn unavailable(_: ArtifactError) -> ApiError {
    ApiError(
        StatusCode::NOT_FOUND,
        "file_unavailable",
        "文件不存在或链接无效",
    )
}
async fn deliver(
    state: MediaState,
    (id, name): (String, String),
    access: Access,
    method: Method,
    headers: HeaderMap,
    preview: bool,
    attachment: bool,
) -> Result<Response, ApiError> {
    let permit = state.io.clone().try_acquire_owned().map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_busy",
            "文件服务暂忙，请稍后重试",
        )
    })?;
    let artifacts = state.artifacts.clone();
    let (record, file, permit) = tokio::task::spawn_blocking(move || {
        artifacts
            .open_signed(&id, &name, access.expires, &access.signature)
            .map(|(record, file)| (record, file, permit))
    })
    .await
    .map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_unavailable",
            "文件服务暂不可用",
        )
    })?
    .map_err(unavailable)?;
    if preview
        && (!record.content_type.starts_with("text/html") || record.size_bytes > 2 * 1024 * 1024)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "preview_unavailable",
            "该文件不支持网页预览，请下载查看",
        ));
    }
    let size = record.size_bytes as u64;
    let etag = format!("\"{}\"", record.sha256);
    let encoded = record
        .filename
        .as_bytes()
        .iter()
        .map(|&v| {
            if v.is_ascii_alphanumeric() || matches!(v, b'.' | b'-' | b'_') {
                (v as char).to_string()
            } else {
                format!("%{v:02X}")
            }
        })
        .collect::<String>();
    let inline = preview
        || (!access.download
            && !attachment
            && (record.content_type.starts_with("image/")
                || record.content_type.starts_with("audio/")
                || record.content_type.starts_with("video/")));
    let mut response = Response::builder()
        .header("content-type", &record.content_type)
        .header("accept-ranges", "bytes")
        .header("etag", &etag)
        .header("cache-control", "private, no-store")
        .header("referrer-policy", "no-referrer")
        .header("x-content-type-options", "nosniff")
        .header("access-control-allow-origin", "*")
        .header(
            "access-control-expose-headers",
            "Content-Length, Content-Range, ETag, Content-Disposition",
        )
        .header(
            "content-disposition",
            format!(
                "{}; filename*=UTF-8''{encoded}",
                if inline { "inline" } else { "attachment" }
            ),
        );
    if preview {
        response=response.header("content-security-policy","sandbox allow-scripts allow-popups allow-popups-to-escape-sandbox; default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data: blob:; font-src data:; media-src data: blob:; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")
            .header("permissions-policy","camera=(), microphone=(), geolocation=(), clipboard-read=(), clipboard-write=()");
    }
    if headers.get("if-none-match").and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return response
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .map_err(|_| unavailable(ArtifactError::Storage));
    }
    let range_value = if headers
        .get("if-range")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value != etag)
    {
        None
    } else {
        headers.get("range").and_then(|v| v.to_str().ok())
    };
    let range = match range::parse(range_value, size) {
        Ok(range) => range,
        Err(()) => {
            return response
                .status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header("content-range", format!("bytes */{size}"))
                .header("content-length", "0")
                .body(Body::empty())
                .map_err(|_| unavailable(ArtifactError::Storage));
        }
    };
    let (start, length) = if let Some((start, end)) = range {
        response = response
            .status(StatusCode::PARTIAL_CONTENT)
            .header("content-range", format!("bytes {start}-{end}/{size}"));
        (start, end - start + 1)
    } else {
        response = response.status(StatusCode::OK);
        (0, size)
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let mut file = tokio::fs::File::from_std(file);
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|_| unavailable(ArtifactError::Storage))?;
        Body::from_stream(super::stream::OwnedStream {
            reader: tokio_util::io::ReaderStream::new(file.take(length)),
            _permit: permit,
        })
    };
    response
        .header("content-length", length)
        .body(body)
        .map_err(|_| unavailable(ArtifactError::Storage))
}
