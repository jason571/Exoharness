// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! True upstream streaming. Only answer text crosses the side-channel; tool
//! proposals are reconstructed and then validated by the ordinary adapter.
mod anthropic;
mod openai;
pub(crate) mod sse;
use crate::{
    ErrorKind,
    adapters::WireRequest,
    errors::{ModelError, Result},
    transport,
};
use serde_json::Value;
use std::{future::Future, pin::Pin};

/// Awaited delivery provides bounded backpressure; failure drops the upstream
/// response immediately. The consumer must not execute anything from a delta.
pub trait TextSink: Send {
    fn push<'a>(
        &'a mut self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = std::result::Result<(), ()>> + Send + 'a>>;
}
pub(crate) async fn send(
    client: &reqwest::Client,
    wire: WireRequest,
    api: &str,
    limit: usize,
    sink: &mut dyn TextSink,
) -> Result<Value> {
    let mut response = transport::checked(
        client
            .post(wire.url)
            .headers(wire.headers)
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&wire.body),
    )
    .await?;
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim();
    if mime != "text/event-stream" {
        // Some compatible servers return ordinary JSON despite stream=true.
        // Validate it normally, but never manufacture fake incremental output.
        return transport::json_response(response, limit).await;
    }
    let mut decoder = sse::Decoder::new(limit);
    let mut collector = match api {
        "openai_compatible" => Collector::Openai(openai::Collector::default()),
        "anthropic" => Collector::Anthropic(anthropic::Collector::default()),
        _ => return Err(ModelError::new(ErrorKind::Unsupported)),
    };
    let mut bytes = 0usize;
    while let Some(chunk) = response.chunk().await.map_err(transport::transport)? {
        bytes = bytes
            .checked_add(chunk.len())
            .filter(|n| *n <= limit)
            .ok_or_else(|| ModelError::new(ErrorKind::OutputLimit))?;
        for frame in decoder.push(&chunk)? {
            let delta = collector.frame(&frame)?;
            if !delta.is_empty() {
                sink.push(&delta)
                    .await
                    .map_err(|()| ModelError::new(ErrorKind::Cancelled))?;
            }
        }
    }
    decoder.finish()?;
    collector.finish()
}
enum Collector {
    Openai(openai::Collector),
    Anthropic(anthropic::Collector),
}
impl Collector {
    fn frame(&mut self, frame: &[u8]) -> Result<String> {
        match self {
            Self::Openai(c) => c.frame(frame),
            Self::Anthropic(c) => c.frame(frame),
        }
    }
    fn finish(self) -> Result<Value> {
        match self {
            Self::Openai(c) => c.finish(),
            Self::Anthropic(c) => c.finish(),
        }
    }
}
fn invalid() -> ModelError {
    ModelError::new(ErrorKind::InvalidResponse)
}
fn json(frame: &[u8]) -> Result<Value> {
    exoharness_contracts::strict_json::parse(std::str::from_utf8(frame).map_err(|_| invalid())?)
        .map_err(|_| invalid())
}
