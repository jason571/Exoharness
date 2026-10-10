use crate::{ConfigurationError, Result};
use serde_json::Value;

pub(super) fn model_type(value: &Value) -> Result<&str> {
    let kind = match value.get("model_type") {
        Some(value) => value
            .as_str()
            .ok_or(ConfigurationError::Invalid("model type must be text"))?,
        None => "llm",
    };
    if !matches!(kind, "llm" | "tts" | "asr") {
        return Err(ConfigurationError::Invalid("unknown model type"));
    }
    Ok(kind)
}
