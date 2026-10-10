// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Project explicit MCP file descriptors into UI cards, never scan prose for links.
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
pub(crate) fn collect(value: &Value) -> Vec<Value> {
    if value["isError"] == true
        || internal(value)
        || value.get("structuredContent").is_some_and(internal)
    {
        return vec![];
    }
    let mut items: Vec<Value> = value
        .pointer("/structuredContent/artifacts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    // Legacy structured results may declare a single file rather than an
    // artifacts array. The same descriptor validation applies to both forms.
    if let Some(result) = value
        .pointer("/structuredContent/result")
        .filter(|result| result.is_object())
    {
        items.push(result.clone());
    }
    for block in value["content"].as_array().into_iter().flatten() {
        if block["type"] == "resource_link" {
            items.push(json!({"url":block["uri"],"filename":block["name"],"content_type":block["mimeType"],"size_bytes":block["size"],"_meta":block["_meta"]}));
        }
    }
    let mut seen = BTreeSet::new();
    items.iter().filter_map(|item| {
        if internal(item) {return None;}
        let url=item["url"].as_str().or_else(||item["download_url"].as_str())?;
        let parsed=url::Url::parse(url).ok()?;
        if !matches!(parsed.scheme(),"http"|"https") || parsed.host_str().is_none() || !parsed.username().is_empty() || parsed.password().is_some() {return None;}
        let filename=item["filename"].as_str()?;
        if filename.is_empty() || filename.len()>240 || filename.chars().any(char::is_control) {return None;}
        let mime=item["content_type"].as_str()?;
        let kind=if mime.starts_with("video/") {"video"} else if mime.starts_with("audio/") {"audio"} else if mime.starts_with("image/") {"image"} else {"file"};
        let identity=item["sha256"].as_str().unwrap_or(url);
        let id=format!("mcp-artifact-{:x}",Sha256::digest(format!("{identity}\0{filename}").as_bytes()));
        if !seen.insert(format!("{url}\0{filename}")) {return None;}
        Some(json!({"id":id,"kind":kind,"filename":filename,"content_type":mime,"url":url,"download_url":item["download_url"].as_str().unwrap_or(url),"sha256":item["sha256"],"size_bytes":item["size_bytes"],"url_ttl_seconds":item["url_ttl_seconds"]}))
    }).collect()
}
fn internal(value: &Value) -> bool {
    value["artifact_visibility"] == "internal"
        || value.pointer("/_meta/exoharness/artifact_visibility") == Some(&json!("internal"))
        || value.pointer("/_meta/exoharness/background_task/artifact_visibility")
            == Some(&json!("internal"))
}
#[cfg(test)]
#[path = "../tests/unit/result_artifacts.rs"]
mod tests;
