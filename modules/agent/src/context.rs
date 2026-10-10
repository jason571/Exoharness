// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_contracts::model::ChatRequest;
use exoharness_core::context::{JournalEntry, ProjectionError};
use serde_json::Value;
use std::collections::BTreeSet;

/// Keep complete user/answer pairs, newest first by priority. Never cut JSON,
/// source code, encoded data or a ToolCall in half to make a request smaller.
pub fn text_request(
    system: &str,
    history: &Value,
    prompt: &str,
    output_limit: u64,
) -> Result<ChatRequest, ProjectionError> {
    let mut selected = Vec::new();
    let mut size = system.len() + prompt.len();
    for turn in history["turns"]
        .as_array()
        .into_iter()
        .flatten()
        .rev()
        .take(8)
    {
        let (Some(input), Some(answer)) = (turn["prompt"].as_str(), turn["answer"].as_str()) else {
            continue;
        };
        if turn["status"] != "succeeded" {
            continue;
        }
        let mut input = input.to_owned();
        if let Some(attachments) = turn["attachments"].as_array().filter(|a| !a.is_empty()) {
            let references: Vec<Value> = attachments
                .iter()
                .map(|a| {
                    let mut reference = serde_json::Map::new();
                    for key in [
                        "type",
                        "name",
                        "mime_type",
                        "size",
                        "attachment_id",
                        "artifact_id",
                        "url",
                        "sha256",
                        "content",
                    ] {
                        if let Some(value) = a.get(key) {
                            reference.insert(key.into(), value.clone());
                        }
                    }
                    Value::Object(reference)
                })
                .collect();
            input.push_str("\nUser-provided attachments (input data):\n");
            input.push_str(&serde_json::json!(references).to_string());
        }
        if input.len() + answer.len() + size > 32 * 1024 {
            break;
        }
        size += input.len() + answer.len();
        selected.push((input, answer.to_owned()));
    }
    let mut entries = Vec::new();
    for (input, answer) in selected.into_iter().rev() {
        entries.push(JournalEntry::User(input));
        entries.push(JournalEntry::Assistant(answer));
    }
    entries.push(JournalEntry::User(prompt.into()));
    crate::project_request(system, &entries, &BTreeSet::new(), vec![], output_limit)
}
