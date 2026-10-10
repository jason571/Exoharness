// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Result, StoreError};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

pub(crate) fn value<T: Serialize>(item: &T) -> Result<Value> {
    Ok(serde_json::to_value(item)?)
}
pub(crate) fn number(value: u64) -> Result<i64> {
    value
        .try_into()
        .map_err(|_| StoreError::Invalid("number exceeds database range"))
}
pub(crate) fn identifier(id: &str) -> Result<()> {
    if id.trim().is_empty() || id.len() > 256 {
        return Err(StoreError::Invalid(
            "identifier must be nonblank and at most 256 bytes",
        ));
    }
    Ok(())
}

pub(crate) fn now_ms() -> Result<i64> {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StoreError::Invalid("system clock precedes epoch"))?
        .as_millis();
    time.try_into()
        .map_err(|_| StoreError::Invalid("system clock exceeds database range"))
}
pub(crate) fn encoded<T: Serialize>(item: &T) -> Result<String> {
    Ok(serde_json::to_string(item)?)
}
pub(crate) fn parsed<T: DeserializeOwned>(text: &str) -> Result<T> {
    Ok(serde_json::from_str(text)?)
}

/// Sort recursively rather than relying on serde_json's optional map feature.
fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let mut out = Map::new();
            for key in keys {
                out.insert(key.clone(), canonical(&object[key]));
            }
            Value::Object(out)
        }
        Value::Array(values) => Value::Array(values.iter().map(canonical).collect()),
        other => other.clone(),
    }
}

pub(crate) fn digest(value: &Value) -> Result<String> {
    let encoded = serde_json::to_vec(&canonical(value))?;
    Ok(format!("{:x}", Sha256::digest(encoded)))
}
