use crate::{
    ConfigurationError, Result,
    validation::{asset, fields, text, unique_ids},
};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn validate(catalog: &Value) -> Result<()> {
    fields(catalog, &["profiles", "default_profile"])?;
    let profiles = catalog
        .get("profiles")
        .and_then(Value::as_array)
        .ok_or(ConfigurationError::Invalid("profiles must be an array"))?;
    let mut ids = BTreeSet::new();
    for record in profiles {
        fields(
            record,
            &[
                "id",
                "enabled",
                "active_revision",
                "revisions",
                "created_at",
                "updated_at",
            ],
        )?;
        let id = text(record, "id")?;
        if !ids.insert(id) {
            return Err(ConfigurationError::Invalid("duplicate assistant ID"));
        }
        if !record.get("enabled").is_some_and(Value::is_boolean) {
            return Err(ConfigurationError::Invalid(
                "assistant enabled must be a boolean",
            ));
        }
        let active = record
            .get("active_revision")
            .and_then(Value::as_u64)
            .filter(|value| *value > 0)
            .ok_or(ConfigurationError::Invalid("invalid active revision"))?;
        for field in ["created_at", "updated_at"] {
            if !record
                .get(field)
                .is_some_and(|value| value.as_u64().is_some())
            {
                return Err(ConfigurationError::Invalid("invalid assistant timestamp"));
            }
        }
        let revisions = record
            .get("revisions")
            .and_then(Value::as_array)
            .ok_or(ConfigurationError::Invalid("revisions must be an array"))?;
        let mut numbers = BTreeSet::new();
        for snapshot in revisions {
            fields(
                snapshot,
                &[
                    "id",
                    "revision",
                    "display_name",
                    "avatar_text",
                    "avatar",
                    "description",
                    "system_prompt",
                    "skill_ids",
                    "access",
                    "default_llm",
                    "digest",
                    "created_at",
                ],
            )?;
            if text(snapshot, "id")? != id {
                return Err(ConfigurationError::Invalid("snapshot identity mismatch"));
            }
            let number = snapshot
                .get("revision")
                .and_then(Value::as_u64)
                .filter(|value| *value > 0)
                .ok_or(ConfigurationError::Invalid("invalid revision"))?;
            if !numbers.insert(number) {
                return Err(ConfigurationError::Invalid("duplicate revision"));
            }
            text(snapshot, "display_name")?;
            text(snapshot, "avatar_text")?;
            text(snapshot, "description")?;
            text(snapshot, "digest")?;
            if snapshot.get("default_llm").is_some_and(|v| !v.is_null()) {
                text(snapshot, "default_llm")?;
            }
            if !snapshot
                .get("system_prompt")
                .and_then(Value::as_str)
                .is_some_and(|value| {
                    !value.trim().is_empty()
                        && value.len() <= 32 * 1024
                        && !value
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
                })
            {
                return Err(ConfigurationError::Invalid("invalid system prompt"));
            }
            if let Some(skills) = snapshot.get("skill_ids") {
                unique_ids(skills)?;
            }
            if let Some(avatar) = snapshot.get("avatar").filter(|value| !value.is_null()) {
                asset(avatar)?;
            }
            if let Some(access) = snapshot.get("access").filter(|value| !value.is_null()) {
                super::access::validate(access)?;
            }
        }
        if !numbers.contains(&active) {
            return Err(ConfigurationError::Invalid("active revision missing"));
        }
    }
    if let Some(default) = catalog
        .get("default_profile")
        .filter(|value| !value.is_null())
        && !profiles
            .iter()
            .any(|profile| profile.get("id") == Some(default) && profile["enabled"] == true)
    {
        return Err(ConfigurationError::Invalid("invalid default assistant"));
    }
    Ok(())
}
