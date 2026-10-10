// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Signed static site delivery; no cookie authority, network proxy or execution.
use super::MediaState;
use crate::errors::ApiError;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{Method, StatusCode},
    response::Response,
};
use tokio::io::AsyncReadExt;
pub(super) async fn get(
    State(state): State<MediaState>,
    Path((id, signature, path)): Path<(String, String, String)>,
    method: Method,
) -> Result<Response, ApiError> {
    let unavailable = || {
        ApiError(
            StatusCode::NOT_FOUND,
            "project_file_unavailable",
            "网站文件不存在或预览链接无效",
        )
    };
    let permit = state.io.clone().try_acquire_owned().map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "delivery_busy",
            "文件服务暂忙",
        )
    })?;
    let artifacts = state.artifacts.clone();
    let (record, file, base) = tokio::task::spawn_blocking(move || {
        let (record, file) = artifacts.open_project_signed(&id, &signature, &path)?;
        Ok::<_, exoharness_artifacts::ArtifactError>((
            record,
            file,
            artifacts.project_base(&id, &signature)?,
        ))
    })
    .await
    .map_err(|_| unavailable())?
    .map_err(|_| unavailable())?;
    let size = record.size_bytes as u64;
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from_stream(super::stream::OwnedStream {
            reader: tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file).take(size)),
            _permit: permit,
        })
    };
    let csp = format!(
        "sandbox allow-scripts; default-src 'none'; script-src 'unsafe-inline' {base}; style-src 'unsafe-inline' {base}; img-src data: blob: {base}; font-src data: {base}; media-src data: blob: {base}; connect-src {base}; object-src 'none'; base-uri 'none'; form-action 'none'; worker-src 'none'"
    );
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", record.content_type)
        .header("content-length", size)
        .header("content-security-policy", csp)
        .header(
            "permissions-policy",
            "camera=(), microphone=(), geolocation=(), clipboard-read=(), clipboard-write=()",
        )
        .header("x-content-type-options", "nosniff")
        .header("referrer-policy", "no-referrer")
        .header("cache-control", "private, no-store")
        .header("access-control-allow-origin", "*")
        .body(body)
        .map_err(|_| unavailable())
}
