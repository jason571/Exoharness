// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Protocol adapters return bytes only. Persistence/delivery belongs to Artifacts.
mod volcengine;
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
};
use exoharness_configuration::ModelBinding;
pub struct SynthesizedAudio {
    pub bytes: Vec<u8>,
    pub extension: String,
}
impl std::fmt::Debug for SynthesizedAudio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SynthesizedAudio")
            .field("bytes_len", &self.bytes.len())
            .field("extension", &self.extension)
            .finish()
    }
}
pub(crate) async fn run(
    client: &reqwest::Client,
    binding: &ModelBinding,
    text: &str,
    request_limit: usize,
    response_limit: usize,
) -> Result<SynthesizedAudio> {
    match (
        binding.model["model_type"].as_str(),
        binding.provider["api"].as_str(),
    ) {
        (Some("tts"), Some("volcengine_tts_sse")) => {
            volcengine::run(client, binding, text, request_limit, response_limit).await
        }
        _ => Err(ModelError::new(ErrorKind::Unsupported)),
    }
}
