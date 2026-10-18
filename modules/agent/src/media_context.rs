// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Inline MCP media belongs to the durable receipt, not a text-only request.
use serde_json::{Value, json};

pub(super) fn project(value: &Value, invocation: &str) -> Value {
    let mut projected = value.clone();
    // Some MCPs return the same JSON as structuredContent and as a text block.
    // Keep the typed value once, without shortening code or losing fields.
    if let Some(structured) = value.get("structuredContent").filter(|v| v.is_object())
        && let Some(content) = projected.get_mut("content").and_then(Value::as_array_mut)
    {
        content.retain(|item| {
            item["type"] != "text"
                || item.as_object().is_none_or(|object| object.len() != 2)
                || item.get("annotations").is_some()
                || item.get("_meta").is_some()
                || item.get("text").and_then(Value::as_str).is_none_or(|text| {
                    serde_json::from_str::<Value>(text).map_or(true, |parsed| parsed != *structured)
                })
        });
    }
    let Some(content) = projected.get_mut("content").and_then(Value::as_array_mut) else {
        return projected;
    };
    for (index, item) in content.iter_mut().enumerate() {
        let Some(object) = item.as_object_mut() else {
            continue;
        };
        let (object, data_key) = match object.get("type").and_then(Value::as_str) {
            Some("image" | "audio") => (object, "data"),
            Some("resource") => {
                let Some(resource) = object.get_mut("resource").and_then(Value::as_object_mut)
                else {
                    continue;
                };
                (resource, "blob")
            }
            _ => continue,
        };
        let Some(data) = object.get(data_key).and_then(Value::as_str) else {
            continue;
        };
        let bytes = data.len();
        object.remove(data_key);
        object.insert("inline_media_reference".into(), json!({
            "invocation_id": invocation,
            "content_index": index,
            "encoding": "base64",
            "encoded_bytes": bytes,
            "source": "persisted original tool receipt",
            "guidance": "Inline media is retained in the original receipt. This text-only request contains its reference, not visual or audio input. Use the existing resource link or delivered file for delivery; use an authorized media-capable tool for inspection. Do not claim to have inspected the media from this reference and do not regenerate a completed result."
        }));
    }
    projected
}
