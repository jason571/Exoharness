// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Declarative selections, never implicit permission grants.
use crate::{
    ConfigurationError, Result,
    validation::{fields, text, unique_ids},
};
use serde_json::Value;

pub(super) fn validate(access: &Value) -> Result<()> {
    fields(
        access,
        &[
            "skills",
            "mcp",
            "tools",
            "models",
            "published",
            "software_engineering",
        ],
    )?;
    if !access.get("published").is_some_and(Value::is_boolean) {
        return Err(ConfigurationError::Invalid("published must be boolean"));
    }
    for group in ["skills", "mcp", "tools", "models"] {
        let selection = access
            .get(group)
            .ok_or(ConfigurationError::Invalid("selection group missing"))?;
        fields(selection, &["mode", "ids"])?;
        match text(selection, "mode")? {
            "selected" => unique_ids(
                selection
                    .get("ids")
                    .ok_or(ConfigurationError::Invalid("selected IDs missing"))?,
            )?,
            "none" | "all" if selection.get("ids").is_none() => {}
            _ => return Err(ConfigurationError::Invalid("invalid selection mode")),
        }
    }
    if let Some(engineering) = access
        .get("software_engineering")
        .filter(|value| !value.is_null())
    {
        fields(engineering, &["runtime_capability_id"])?;
        text(engineering, "runtime_capability_id")?;
    }
    Ok(())
}
