// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Actual multipart/silent-WAV protocol check; not recognition accuracy proof.
use super::DiagnosticReport;
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
    transport,
};
use exoharness_configuration::ModelBinding;
pub(super) async fn test(
    client: &reqwest::Client,
    binding: &ModelBinding,
    request_limit: usize,
    limit: usize,
) -> Result<DiagnosticReport> {
    let base = binding.provider["base_url"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let model = binding.model["model_id"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let wav = silent_wav();
    // Include conservative multipart framing overhead before allocating/sending.
    if wav.len().saturating_add(model.len()).saturating_add(1024) > request_limit {
        return Err(ModelError::new(ErrorKind::InvalidRequest));
    }
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("model-test.wav")
        .mime_str("audio/wav")
        .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?;
    let form = reqwest::multipart::Form::new()
        .text("model", model.to_owned())
        .text("response_format", "json")
        .part("file", part);
    let request = client
        .post(format!(
            "{}/audio/transcriptions",
            base.trim_end_matches('/')
        ))
        .multipart(form);
    let request = binding.with_credential(|key| match key {
        Some(key) => request.bearer_auth(key),
        None => request,
    });
    let response = transport::checked(request).await?;
    let value = transport::json_response(response, limit).await?;
    if value.get("error").is_some_and(|v| !v.is_null()) || !value["text"].is_string() {
        return Err(ModelError::new(ErrorKind::InvalidResponse));
    }
    Ok(DiagnosticReport::new(binding, "transcription_protocol"))
}
fn silent_wav() -> Vec<u8> {
    let mut wav = Vec::from(&b"RIFF"[..]);
    wav.extend_from_slice(&32036u32.to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&32000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&32000u32.to_le_bytes());
    wav.resize(32044, 0);
    wav
}
