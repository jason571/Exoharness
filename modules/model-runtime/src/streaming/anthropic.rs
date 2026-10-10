// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{invalid, json};
use crate::{
    ErrorKind,
    errors::{ModelError, Result},
};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Default)]
pub(super) struct Collector {
    message: Option<Value>,
    blocks: BTreeMap<u64, Value>,
    args: BTreeMap<u64, String>,
    closed: BTreeSet<u64>,
    done: bool,
}
impl Collector {
    pub fn frame(&mut self, frame: &[u8]) -> Result<String> {
        let value = json(frame)?;
        if self.done {
            return Err(invalid());
        }
        let mut text = String::new();
        match value["type"].as_str() {
            Some("ping") => {}
            Some("error") => return Err(ModelError::new(ErrorKind::ProviderRejected)),
            Some("message_start") if self.message.is_none() => {
                if value["message"]["role"] != "assistant" || value["message"]["type"] != "message"
                {
                    return Err(invalid());
                }
                self.message = Some(value["message"].clone());
            }
            Some("content_block_start") if self.message.is_some() => {
                let index = value["index"]
                    .as_u64()
                    .filter(|i| *i < 256)
                    .ok_or_else(invalid)?;
                let block = value["content_block"].clone();
                if !matches!(block["type"].as_str(), Some("text" | "tool_use"))
                    || self.blocks.contains_key(&index)
                {
                    return Err(invalid());
                }
                if block["type"] == "text" {
                    text = block["text"].as_str().ok_or_else(invalid)?.into();
                }
                self.blocks.insert(index, block);
            }
            Some("content_block_delta") => {
                let index = value["index"].as_u64().ok_or_else(invalid)?;
                if self.closed.contains(&index) {
                    return Err(invalid());
                }
                let block = self.blocks.get_mut(&index).ok_or_else(invalid)?;
                match (block["type"].as_str(), value["delta"]["type"].as_str()) {
                    (Some("text"), Some("text_delta")) => {
                        text = value["delta"]["text"].as_str().ok_or_else(invalid)?.into();
                        let old = block["text"].as_str().ok_or_else(invalid)?;
                        block["text"] = json!(format!("{old}{text}"));
                    }
                    (Some("tool_use"), Some("input_json_delta")) => {
                        self.args.entry(index).or_default().push_str(
                            value["delta"]["partial_json"]
                                .as_str()
                                .ok_or_else(invalid)?,
                        )
                    }
                    _ => return Err(invalid()),
                }
            }
            Some("content_block_stop") => {
                let index = value["index"].as_u64().ok_or_else(invalid)?;
                let block = self.blocks.get_mut(&index).ok_or_else(invalid)?;
                if !self.closed.insert(index) {
                    return Err(invalid());
                }
                if let Some(args) = self.args.remove(&index) {
                    block["input"] = json(args.as_bytes())?;
                }
            }
            Some("message_delta") => {
                if self.closed.len() != self.blocks.len() {
                    return Err(invalid());
                }
                let message = self.message.as_mut().ok_or_else(invalid)?;
                message["stop_reason"] = value["delta"]["stop_reason"].clone();
                if let Some(usage) = value["usage"].as_object() {
                    for (key, value) in usage {
                        message["usage"][key] = value.clone();
                    }
                }
            }
            Some("message_stop")
                if self.message.is_some() && self.closed.len() == self.blocks.len() =>
            {
                self.done = true
            }
            _ => return Err(invalid()),
        }
        Ok(text)
    }
    pub fn finish(self) -> Result<Value> {
        if !self.done {
            return Err(ModelError::new(ErrorKind::Incomplete));
        }
        let mut value = self.message.ok_or_else(invalid)?;
        value["content"] = json!(self.blocks.into_values().collect::<Vec<_>>());
        Ok(value)
    }
}
