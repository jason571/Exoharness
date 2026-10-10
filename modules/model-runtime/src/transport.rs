// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! One asynchronous HTTP attempt, no redirects/proxy inference/hidden retries.
use crate::{
    ErrorKind,
    adapters::WireRequest,
    errors::{ModelError, Result},
};
use exoharness_contracts::strict_json;
use reqwest::{Client, Response};
use serde_json::Value;
use std::time::Duration;

pub(crate) fn client() -> Result<Client> {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|_| ModelError::new(ErrorKind::Configuration))
}
pub(crate) async fn send(client: &Client, request: WireRequest, limit: usize) -> Result<Value> {
    let bytes = serde_json::to_vec(&request.body)
        .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?;
    let response = checked(
        client
            .post(request.url)
            .headers(request.headers)
            .header("content-type", "application/json")
            .header("accept", "application/json")
            .body(bytes),
    )
    .await?;
    json_response(response, limit).await
}
pub(crate) async fn checked(request: reqwest::RequestBuilder) -> Result<Response> {
    let response = request.send().await.map_err(transport)?;
    if !response.status().is_success() {
        let mut error = ModelError::new(ErrorKind::Http);
        error.status = Some(response.status().as_u16());
        error.retry_after_seconds = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        return Err(error);
    }
    Ok(response)
}
pub(crate) async fn json_response(response: Response, limit: usize) -> Result<Value> {
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    if content_type != "application/json" && !content_type.ends_with("+json") {
        return Err(ModelError::new(ErrorKind::InvalidResponse));
    }
    let bytes = read(response, limit).await?;
    strict_json::parse(
        std::str::from_utf8(&bytes).map_err(|_| ModelError::new(ErrorKind::InvalidResponse))?,
    )
    .map_err(|_| ModelError::new(ErrorKind::InvalidResponse))
}
pub(crate) fn transport(error: reqwest::Error) -> ModelError {
    ModelError::new(if error.is_timeout() {
        ErrorKind::Timeout
    } else {
        ErrorKind::Transport
    })
}
async fn read(mut response: Response, limit: usize) -> Result<Vec<u8>> {
    if response.content_length().is_some_and(|v| v > limit as u64) {
        return Err(ModelError::new(ErrorKind::OutputLimit));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport)? {
        if chunk.len() > limit.saturating_sub(bytes.len()) {
            return Err(ModelError::new(ErrorKind::OutputLimit));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
