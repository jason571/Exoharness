// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Separate private management views from public identity fields.
use super::Assistants;
use serde_json::{Value, json};

impl Assistants {
    pub fn list(&self) -> Vec<Value> {
        let catalog = self.catalog();
        let mut result = Vec::new();
        for record in catalog["profiles"].as_array().into_iter().flatten() {
            if let Some(active) = record["revisions"]
                .as_array()
                .into_iter()
                .flatten()
                .find(|revision| revision["revision"] == record["active_revision"])
            {
                let mut value = active.clone();
                let object = value.as_object_mut().expect("validated snapshot object");
                object.remove("revision");
                for field in ["enabled", "active_revision", "created_at", "updated_at"] {
                    object.insert(field.into(), record[field].clone());
                }
                object.insert(
                    "is_default".into(),
                    json!(catalog.get("default_profile") == record.get("id")),
                );
                object.entry("skill_ids").or_insert(json!([]));
                object.entry("access").or_insert(Value::Null);
                result.push(value);
            }
        }
        result.sort_by(|a, b| {
            b["is_default"]
                .as_bool()
                .cmp(&a["is_default"].as_bool())
                .then_with(|| a["display_name"].as_str().cmp(&b["display_name"].as_str()))
        });
        result
    }
    pub fn detail(&self, id: &str) -> Option<Value> {
        self.list()
            .into_iter()
            .find(|value| value["id"].as_str() == Some(id))
    }
    pub fn default_public(&self) -> Value {
        let Some(profile) = self
            .list()
            .into_iter()
            .find(|value| value["is_default"] == true && value["enabled"] == true)
        else {
            return Value::Null;
        };
        self.public_profile(profile["id"].as_str().expect("validated profile id"))
    }
    pub fn public_profile(&self, id: &str) -> Value {
        let Some(profile) = self.detail(id).filter(|profile| {
            profile["enabled"] == true
                && (profile["is_default"] == true
                    || profile.pointer("/access/published") == Some(&json!(true)))
        }) else {
            return Value::Null;
        };
        project_identity(&profile)
    }
    /// Identity projection for an assistant explicitly assigned by an
    /// administrator to an external application. Portal publication is separate.
    pub fn delegated_profile(&self, id: &str) -> Value {
        self.detail(id)
            .filter(|profile| profile["enabled"] == true)
            .map(|profile| project_identity(&profile))
            .unwrap_or(Value::Null)
    }
}

fn project_identity(profile: &Value) -> Value {
    let mut result = serde_json::Map::new();
    for field in [
        "id",
        "display_name",
        "avatar_text",
        "avatar",
        "description",
        "digest",
    ] {
        if let Some(value) = profile.get(field) {
            result.insert(field.into(), value.clone());
        }
    }
    result.insert("revision".into(), profile["active_revision"].clone());
    result.insert(
        "software_engineering".into(),
        json!(
            profile
                .pointer("/access/software_engineering")
                .is_some_and(|value| !value.is_null())
        ),
    );
    Value::Object(result)
}
