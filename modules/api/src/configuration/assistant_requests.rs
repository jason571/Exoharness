// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::model_requests;
use crate::errors::ApiError;
use axum::body::Bytes;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Write {
    pub id: Option<String>,
    pub expected_revision: Option<u64>,
    pub enabled: Option<bool>,
    pub make_default: Option<bool>,
    display_name: String,
    avatar_text: String,
    avatar: Option<Value>,
    description: String,
    system_prompt: String,
    #[serde(default)]
    skill_ids: Vec<String>,
    access: Option<Value>,
    #[serde(default, deserialize_with = "present")]
    default_llm: Option<Value>,
    pub actor: Option<String>,
}
impl Write {
    pub fn input(&self) -> Value {
        let mut input = json!({"display_name":self.display_name,"avatar_text":self.avatar_text,"avatar":self.avatar,"description":self.description,"system_prompt":self.system_prompt,"skill_ids":self.skill_ids,"access":self.access});
        if let Some(value) = &self.default_llm {
            input["default_llm"] = value.clone();
        }
        input
    }
}
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Avatar {
    pub content_type: String,
    pub content_base64: String,
    pub actor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Enabled {
    pub enabled: bool,
    pub actor: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Action {
    #[serde(default)]
    pub confirm: bool,
    pub actor: Option<String>,
}
pub(super) fn parse<T: serde::de::DeserializeOwned>(body: Bytes) -> Result<T, ApiError> {
    model_requests::parse(body)
}
