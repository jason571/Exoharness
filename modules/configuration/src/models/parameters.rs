// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Model limits, sampling and provider relationship validation.
use super::kinds::model_type;
use crate::{
    ConfigurationError, Result,
    validation::{boolean, fields, text},
};
use serde_json::Value;
use std::collections::BTreeSet;

/// Original API defaults, not detected upstream capabilities or model limits.
/// Only omitted fields are filled; null/string/invalid values are still rejected.
pub(super) fn defaults(value: &mut Value) -> Result<()> {
    let object = value.as_object_mut().ok_or(ConfigurationError::Json)?;
    for (field, default) in [
        ("model_type", serde_json::json!("llm")),
        ("context_window", serde_json::json!(128_000)),
        ("max_tokens", serde_json::json!(16_384)),
        ("thinking", serde_json::json!("none")),
        ("enabled", serde_json::json!(true)),
    ] {
        object.entry(field).or_insert(default);
    }
    Ok(())
}

pub(super) fn validate(models: &[Value], providers: &[Value]) -> Result<()> {
    let mut names = BTreeSet::new();
    let mut active = BTreeSet::new();
    for model in models {
        fields(
            model,
            &[
                "name",
                "model_id",
                "display_name",
                "provider",
                "model_type",
                "context_window",
                "max_tokens",
                "thinking",
                "temperature",
                "top_p",
                "top_k",
                "tts",
                "enabled",
            ],
        )?;
        let name = text(model, "name")?;
        if !names.insert(name) {
            return Err(ConfigurationError::Invalid("duplicate model"));
        }
        text(model, "model_id")?;
        text(model, "display_name")?;
        boolean(model, "enabled")?;
        let kind = model_type(model)?;
        if model
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            && !active.insert(kind)
        {
            return Err(ConfigurationError::Invalid(
                "multiple active models for one type",
            ));
        }
        let provider = providers
            .iter()
            .find(|provider| provider["name"].as_str() == model["provider"].as_str())
            .ok_or(ConfigurationError::Invalid("model provider missing"))?;
        if kind != model_type(provider)? {
            return Err(ConfigurationError::Invalid(
                "model and provider type mismatch",
            ));
        }
        for field in ["context_window", "max_tokens"] {
            if let Some(value) = model.get(field)
                && !value.as_u64().is_some_and(|number| number > 0)
            {
                return Err(ConfigurationError::Invalid("invalid token limit"));
            }
        }
        for (field, lower, upper) in [("temperature", 0.0, 2.0), ("top_p", f64::MIN_POSITIVE, 1.0)]
        {
            if let Some(value) = model.get(field).filter(|value| !value.is_null())
                && !value
                    .as_f64()
                    .is_some_and(|number| number.is_finite() && number >= lower && number <= upper)
            {
                return Err(ConfigurationError::Invalid("invalid sampling parameter"));
            }
        }
        if let Some(value) = model.get("top_k").filter(|value| !value.is_null())
            && !value
                .as_u64()
                .is_some_and(|value| (1..=1000).contains(&value))
        {
            return Err(ConfigurationError::Invalid("invalid top_k"));
        }
        if let Some(thinking) = model.get("thinking")
            && !matches!(
                thinking.as_str(),
                Some("none" | "openai" | "anthropic" | "qwen")
            )
        {
            return Err(ConfigurationError::Invalid("invalid thinking mode"));
        }
        if kind == "tts" {
            super::speech::validate(model)?;
        } else if model.get("tts").is_some_and(|value| !value.is_null()) {
            return Err(ConfigurationError::Invalid(
                "TTS settings in another model type",
            ));
        }
    }
    Ok(())
}
