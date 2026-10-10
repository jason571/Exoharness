// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! TTS configuration fields; synthesis belongs to model-runtime adapters.
use crate::{
    ConfigurationError, Result,
    validation::{fields, text},
};
use serde_json::Value;

pub(super) fn validate(model: &Value) -> Result<()> {
    let tts = model
        .get("tts")
        .ok_or(ConfigurationError::Invalid("TTS settings missing"))?;
    fields(
        tts,
        &[
            "profile_id",
            "speaker",
            "language_type",
            "audio_format",
            "sample_rate",
        ],
    )?;
    text(tts, "profile_id")?;
    text(tts, "speaker")?;
    if let Some(value) = tts.get("audio_format").filter(|value| !value.is_null())
        && !matches!(value.as_str(), Some("mp3" | "wav" | "pcm"))
    {
        return Err(ConfigurationError::Invalid("invalid audio format"));
    }
    if let Some(value) = tts.get("sample_rate").filter(|value| !value.is_null())
        && !value
            .as_u64()
            .is_some_and(|rate| (8000..=96000).contains(&rate))
    {
        return Err(ConfigurationError::Invalid("invalid sample rate"));
    }
    Ok(())
}
