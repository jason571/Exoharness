// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{ConfigurationError, Result};
use serde_json::Value;
use std::collections::BTreeSet;

pub(crate) fn fields(value: &Value, allowed: &[&str]) -> Result<()> {
    if !value
        .as_object()
        .is_some_and(|object| object.keys().all(|key| allowed.contains(&key.as_str())))
    {
        return Err(ConfigurationError::Invalid(
            "unknown or secret-valued configuration field",
        ));
    }
    Ok(())
}
pub(crate) fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| {
            !text.trim().is_empty() && text.len() <= 512 && !text.chars().any(char::is_control)
        })
        .ok_or(ConfigurationError::Invalid("invalid text field"))
}
pub(crate) fn boolean(value: &Value, key: &str) -> Result<()> {
    if value.get(key).is_some_and(|field| !field.is_boolean()) {
        return Err(ConfigurationError::Invalid("boolean must be native JSON"));
    }
    Ok(())
}
pub(crate) fn unique_ids(value: &Value) -> Result<()> {
    let items = value.as_array().ok_or(ConfigurationError::Invalid(
        "selection IDs must be an array",
    ))?;
    let mut seen = BTreeSet::new();
    if items.len() > 4096 {
        return Err(ConfigurationError::Invalid("too many selected IDs"));
    }
    for item in items {
        let id = item
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 512 && !id.chars().any(char::is_control))
            .ok_or(ConfigurationError::Invalid("invalid selected ID"))?;
        if !seen.insert(id) {
            return Err(ConfigurationError::Invalid("duplicate selected ID"));
        }
    }
    Ok(())
}
pub(crate) fn asset(value: &Value) -> Result<(&str, &str)> {
    fields(value, &["asset_id", "content_type"])?;
    let id = text(value, "asset_id")?;
    let kind = text(value, "content_type")?;
    let (digest, extension) = id
        .split_once('.')
        .ok_or(ConfigurationError::Invalid("invalid avatar ID"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || !matches!(
            (extension, kind),
            ("png", "image/png") | ("jpg", "image/jpeg") | ("webp", "image/webp")
        )
    {
        return Err(ConfigurationError::Invalid("invalid avatar ID or MIME"));
    }
    Ok((id, kind))
}
