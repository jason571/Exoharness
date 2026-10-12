// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Submission and retrieval receipts may describe the same delivered file.
use serde_json::Value;

fn same_file(left: &Value, right: &Value) -> bool {
    let id = left["id"].as_str().filter(|id| !id.is_empty());
    if id.is_some() && id == right["id"].as_str() {
        return true;
    }
    let digest = left["sha256"].as_str().filter(|digest| !digest.is_empty());
    if digest.is_some()
        && digest == right["sha256"].as_str()
        && left["filename"] == right["filename"]
        && left["content_type"] == right["content_type"]
    {
        return true;
    }
    let url = left["url"].as_str().filter(|url| !url.is_empty());
    url.is_some() && url == right["url"].as_str() && left["filename"] == right["filename"]
}

pub(super) fn append(existing: &mut Vec<Value>, additions: Vec<Value>) {
    for artifact in additions {
        if !existing.iter().any(|current| same_file(current, &artifact)) {
            existing.push(artifact);
        }
    }
}

#[cfg(test)]
#[path = "../../tests/unit/chat_artifact_collection.rs"]
mod tests;
