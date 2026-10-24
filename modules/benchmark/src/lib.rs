// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! Independent Rust evaluation, with frozen inputs and durable submission receipts.
pub mod campaign;
mod canonical;
pub mod cli;
pub mod contracts;
pub mod evidence;
pub mod gdpevo;
pub mod harness;
pub mod metrics;
pub mod model_text;
pub mod scenarios;
pub mod verifiers;

pub use contracts::{Experience, Suite, digest, identifier};
pub use harness::{BenchmarkHarness, Harness, Management, ManagementPort};
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("Evaluation file operation failed")]
    Io(#[from] std::io::Error),
    #[error("Invalid evaluation JSON")]
    Json(#[from] serde_json::Error),
    #[error("Evaluation HTTP transport failed; inspect saved submission intent")]
    Http(#[from] reqwest::Error),
}
pub fn ensure(ok: bool, message: &str) -> Result<()> {
    if ok {
        Ok(())
    } else {
        Err(Error::Invalid(message.into()))
    }
}
pub fn list(v: &serde_json::Value) -> Result<&Vec<serde_json::Value>> {
    v.as_array()
        .ok_or_else(|| Error::Invalid("Expected a JSON list".into()))
}
pub fn text(v: &serde_json::Value) -> Result<&str> {
    v.as_str()
        .ok_or_else(|| Error::Invalid("Expected a JSON string".into()))
}
pub fn number(v: &serde_json::Value) -> Result<u64> {
    v.as_u64()
        .ok_or_else(|| Error::Invalid("Expected a nonnegative integer".into()))
}
pub fn encoded(value: &str) -> String {
    percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC).to_string()
}
/// Durable rejection metadata may contain a structured receipt, not a boolean.
pub fn flagged(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Array(value) => !value.is_empty(),
        serde_json::Value::Object(value) => !value.is_empty(),
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|v| v != 0.0),
    }
}
