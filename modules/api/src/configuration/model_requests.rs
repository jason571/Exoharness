use crate::errors::ApiError;
use axum::{body::Bytes, http::StatusCode};
use exoharness_contracts::strict_json;
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::Value;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Configuration {
    pub provider: Value,
    pub model: Value,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Provider {
    pub provider: Value,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Model {
    pub model: Value,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Enabled {
    pub enabled: bool,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Action {
    #[serde(default)]
    pub confirm: bool,
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Test {
    pub confirm: bool,
    #[serde(default)]
    pub actor: Option<String>,
}
pub(super) fn parse<T: DeserializeOwned>(bytes: Bytes) -> Result<T, ApiError> {
    let value = std::str::from_utf8(&bytes)
        .ok()
        .and_then(|s| strict_json::parse(s).ok())
        .ok_or(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_configuration_json",
            "配置需使用原生 JSON，不能包含重复字段",
        ))?;
    serde_json::from_value(value).map_err(|_| {
        ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_configuration_input",
            "配置字段或类型不正确，请检查输入",
        )
    })
}
