// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bounded OpenAI transcription transport. Audio never enters chat context.
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
    transport,
};
use exoharness_configuration::ModelBinding;

pub const MAX_AUDIO_BYTES: usize = 8 * 1024 * 1024;
pub(crate) fn format(mime: &str) -> Result<(&'static str, &'static str)> {
    match mime.split(';').next().unwrap_or_default().trim() {
        "audio/webm" => Ok(("recording.webm", "audio/webm")),
        "audio/ogg" => Ok(("recording.ogg", "audio/ogg")),
        "audio/wav" | "audio/x-wav" => Ok(("recording.wav", "audio/wav")),
        "audio/mp4" => Ok(("recording.m4a", "audio/mp4")),
        "audio/mpeg" => Ok(("recording.mp3", "audio/mpeg")),
        _ => Err(ModelError::new(ErrorKind::InvalidRequest)),
    }
}
fn endpoint(binding: &ModelBinding, path: &str) -> Result<String> {
    if binding.model["model_type"] != "asr" || binding.provider["api"] != "openai_transcription" {
        return Err(ModelError::new(ErrorKind::Unsupported));
    }
    let base = binding.provider["base_url"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    Ok(format!("{}/{path}", base.trim_end_matches('/')))
}
pub(crate) async fn probe(client: &reqwest::Client, binding: &ModelBinding) -> Result<()> {
    let request = client.get(endpoint(binding, "models")?);
    let request = binding.with_credential(|key| match key {
        Some(key) => request.bearer_auth(key),
        None => request,
    });
    let value = transport::json_response(transport::checked(request).await?, 64 * 1024).await?;
    let model = binding.model["model_id"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    if !value["data"]
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item["id"] == model))
    {
        return Err(ModelError::new(ErrorKind::InvalidResponse));
    }
    Ok(())
}
pub(crate) async fn run(
    client: &reqwest::Client,
    binding: &ModelBinding,
    audio: Vec<u8>,
    mime: &str,
    limit: usize,
) -> Result<String> {
    if audio.is_empty() || audio.len() > MAX_AUDIO_BYTES {
        return Err(ModelError::new(ErrorKind::InvalidRequest));
    }
    let (filename, mime) = format(mime)?;
    let url = endpoint(binding, "audio/transcriptions")?;
    let model = binding.model["model_id"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let part = reqwest::multipart::Part::bytes(audio)
        .file_name(filename)
        .mime_str(mime)
        .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?;
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_owned())
        .text("response_format", "json")
        .part("file", part);
    let request = client.post(url).multipart(form);
    let request = binding.with_credential(|key| match key {
        Some(key) => request.bearer_auth(key),
        None => request,
    });
    let value = transport::json_response(transport::checked(request).await?, limit).await?;
    if value.get("error").is_some_and(|value| !value.is_null()) {
        return Err(ModelError::new(ErrorKind::ProviderRejected));
    }
    value["text"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| ModelError::new(ErrorKind::InvalidResponse))
}
