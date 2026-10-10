use super::{invalid, json};
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct Collector {
    id: Option<String>,
    text: String,
    calls: BTreeMap<u64, Value>,
    reason: Option<String>,
    usage: Value,
    done: bool,
}
impl Collector {
    pub fn frame(&mut self, frame: &[u8]) -> Result<String> {
        if self.done {
            return Err(invalid());
        }
        if frame == b"[DONE]" {
            self.done = true;
            return Ok(String::new());
        }
        let value = json(frame)?;
        if value.get("error").is_some_and(|e| !e.is_null()) {
            return Err(ModelError::new(ErrorKind::ProviderRejected));
        }
        if let Some(id) = value["id"].as_str() {
            if self.id.as_deref().is_some_and(|old| old != id) {
                return Err(invalid());
            }
            self.id = Some(id.into());
        }
        if let Some(usage) = value.get("usage").filter(|u| !u.is_null()) {
            self.usage = usage.clone();
        }
        let choices = value["choices"].as_array().ok_or_else(invalid)?;
        if choices.is_empty() {
            return Ok(String::new());
        }
        if choices.len() != 1 || choices[0]["index"].as_u64() != Some(0) {
            return Err(invalid());
        }
        let choice = &choices[0];
        let delta = &choice["delta"];
        if !delta.is_object() || delta.get("role").is_some_and(|r| r != "assistant") {
            return Err(invalid());
        }
        let text = match delta.get("content") {
            None | Some(Value::Null) => "",
            Some(Value::String(s)) => s,
            _ => return Err(invalid()),
        };
        if self.reason.is_some() && (!text.is_empty() || delta.get("tool_calls").is_some()) {
            return Err(invalid());
        }
        self.text.push_str(text);
        if let Some(calls) = delta.get("tool_calls").filter(|v| !v.is_null()) {
            for call in calls.as_array().ok_or_else(invalid)? {
                let index = call["index"]
                    .as_u64()
                    .filter(|i| *i < 128)
                    .ok_or_else(invalid)?;
                let target = self.calls.entry(index).or_insert_with(
                    || json!({"id":"","type":"function","function":{"name":"","arguments":""}}),
                );
                if let Some(kind) = call.get("type")
                    && kind != "function"
                {
                    return Err(invalid());
                }
                if let Some(id) = call.get("id") {
                    let id = id.as_str().ok_or_else(invalid)?;
                    if target["id"]
                        .as_str()
                        .is_some_and(|old| !old.is_empty() && old != id)
                    {
                        return Err(invalid());
                    }
                    target["id"] = json!(id);
                }
                for field in ["name", "arguments"] {
                    if let Some(part) = call["function"].get(field) {
                        let part = part.as_str().ok_or_else(invalid)?;
                        let previous = target["function"][field].as_str().ok_or_else(invalid)?;
                        target["function"][field] = json!(format!("{previous}{part}"));
                    }
                }
            }
        }
        if let Some(reason) = choice.get("finish_reason").filter(|v| !v.is_null()) {
            if self.reason.is_some() {
                return Err(invalid());
            }
            self.reason = Some(reason.as_str().ok_or_else(invalid)?.into());
        }
        // reasoning_content/reasoning are intentionally not answer deltas.
        Ok(text.into())
    }
    pub fn finish(self) -> Result<Value> {
        if !self.done || self.reason.is_none() {
            return Err(ModelError::new(ErrorKind::Incomplete));
        }
        Ok(
            json!({"id":self.id,"choices":[{"message":{"role":"assistant","content":self.text,"tool_calls":self.calls.into_values().collect::<Vec<_>>()},"finish_reason":self.reason}],"usage":self.usage}),
        )
    }
}
