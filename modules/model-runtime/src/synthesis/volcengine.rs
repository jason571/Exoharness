// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! V3 SSE synthesis; collect real audio only after a successful end event.
use super::SynthesizedAudio;
use crate::streaming::sse::Decoder;
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
    transport,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_configuration::ModelBinding;
use serde_json::{Value, json};
pub(super) async fn run(
    client: &reqwest::Client,
    binding: &ModelBinding,
    text: &str,
    request_limit: usize,
    limit: usize,
) -> Result<SynthesizedAudio> {
    let settings = &binding.model["tts"];
    let speaker = settings["speaker"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let format = settings["audio_format"].as_str().unwrap_or("mp3");
    if !matches!(format, "mp3" | "wav") {
        return Err(ModelError::new(ErrorKind::Unsupported));
    }
    let url = binding.provider["base_url"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let body=serde_json::to_vec(&json!({"user":{"uid":settings["profile_id"]},"req_params":{"text":text,"speaker":speaker,
            "audio_params":{"format":format,"sample_rate":settings["sample_rate"].as_u64().unwrap_or(24000)}}}))
        .map_err(|_|ModelError::new(ErrorKind::InvalidRequest))?;
    if body.len() > request_limit {
        return Err(ModelError::new(ErrorKind::InvalidRequest));
    }
    let request = client
        .post(url)
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .header("X-Api-Request-Id", uuid::Uuid::new_v4().to_string())
        .header(
            "X-Api-Resource-Id",
            binding.model["model_id"]
                .as_str()
                .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?,
        )
        .body(body);
    let request = binding.with_credential(|key| -> Result<_> {
        let key = key.ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
        let mut header = reqwest::header::HeaderValue::from_str(key)
            .map_err(|_| ModelError::new(ErrorKind::Configuration))?;
        header.set_sensitive(true);
        Ok(request.header("X-Api-Key", header))
    })?;
    let mut response = transport::checked(request).await?;
    if response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        != "text/event-stream"
    {
        return Err(ModelError::new(ErrorKind::InvalidResponse));
    }
    let mut decoder = Decoder::new(limit);
    let mut received = 0usize;
    let mut audio = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport::transport)? {
        if chunk.len() > limit.saturating_sub(received) {
            return Err(ModelError::new(ErrorKind::OutputLimit));
        }
        received += chunk.len();
        for frame in decoder.push(&chunk)? {
            if event(&frame, &mut audio, limit)? {
                return complete(format, audio);
            }
        }
    }
    for frame in decoder.finish()? {
        if event(&frame, &mut audio, limit)? {
            return complete(format, audio);
        }
    }
    Err(ModelError::new(ErrorKind::Incomplete))
}
fn complete(format: &str, audio: Vec<u8>) -> Result<SynthesizedAudio> {
    if audio.is_empty() {
        return Err(ModelError::new(ErrorKind::Empty));
    }
    Ok(SynthesizedAudio {
        bytes: audio,
        extension: format.into(),
    })
}
fn event(bytes: &[u8], audio: &mut Vec<u8>, limit: usize) -> Result<bool> {
    let value: Value = exoharness_contracts::strict_json::parse(
        std::str::from_utf8(bytes).map_err(|_| ModelError::new(ErrorKind::InvalidResponse))?,
    )
    .map_err(|_| ModelError::new(ErrorKind::InvalidResponse))?;
    let code = value["code"]
        .as_i64()
        .ok_or_else(|| ModelError::new(ErrorKind::InvalidResponse))?;
    if code != 0 && code != 20_000_000 {
        return Err(ModelError::new(ErrorKind::ProviderRejected));
    }
    if let Some(data) = value.get("data").filter(|v| !v.is_null()) {
        let data = data
            .as_str()
            .ok_or_else(|| ModelError::new(ErrorKind::InvalidResponse))?;
        let bytes = STANDARD
            .decode(data)
            .map_err(|_| ModelError::new(ErrorKind::InvalidResponse))?;
        if bytes.len() > limit.saturating_sub(audio.len()) {
            return Err(ModelError::new(ErrorKind::OutputLimit));
        }
        audio.extend_from_slice(&bytes);
    }
    Ok(code == 20_000_000)
}
