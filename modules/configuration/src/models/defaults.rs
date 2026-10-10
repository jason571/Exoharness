// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::kinds::model_type;
use crate::{ConfigurationError, Result};
use serde_json::Value;

pub(super) fn validate(catalog: &Value, models: &[Value]) -> Result<()> {
    if let Some(defaults) = catalog.get("default_models") {
        let defaults = defaults
            .as_object()
            .ok_or(ConfigurationError::Invalid("defaults must be an object"))?;
        for (kind, name) in defaults {
            if !models.iter().any(|model| {
                model["name"] == *name && model_type(model).ok() == Some(kind.as_str())
            }) {
                return Err(ConfigurationError::Invalid("invalid default model"));
            }
        }
    }
    if let Some(default) = catalog
        .get("default_model")
        .filter(|value| !value.is_null())
    {
        if !models
            .iter()
            .any(|model| model["name"] == *default && model_type(model).ok() == Some("llm"))
        {
            return Err(ConfigurationError::Invalid("invalid LLM default"));
        }
        if let Some(other) = catalog.pointer("/default_models/llm")
            && other != default
        {
            return Err(ConfigurationError::Invalid("conflicting LLM defaults"));
        }
    }
    Ok(())
}
