// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bound historical read bodies only when the host declared durable storage.
use exoharness_core::context::{ContextItem, JournalEntry};
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn compact_previous_reads(items: &mut [ContextItem], entries: &[JournalEntry]) {
    let verified: BTreeSet<_> = items
        .iter()
        .filter_map(|item| match item {
            ContextItem::Checkpoint { invocation_id, .. } => Some(invocation_id.as_str()),
            _ => None,
        })
        .collect();
    let mut authored = BTreeSet::new();
    let mut eligible = BTreeSet::new();
    for entry in entries {
        let JournalEntry::ToolExchange {
            invocation_id,
            arguments,
            result,
            ..
        } = entry
        else {
            continue;
        };
        if !verified.contains(invocation_id.as_str()) {
            continue;
        }
        let Some(workspace) = result["workspace_id"].as_str() else {
            continue;
        };
        // Only complete source the model actually authored in this Run qualifies.
        // Reading uploads or inherited project files must not discard their data.
        if let Some(files) = arguments["files"].as_object() {
            for (path, content) in files {
                if content.is_string() {
                    authored.insert((workspace.to_owned(), path.to_owned()));
                }
            }
        }
        if let Some(path) = result["file"]["path"].as_str()
            && authored.contains(&(workspace.to_owned(), path.to_owned()))
        {
            eligible.insert(invocation_id.as_str());
        }
    }
    let mut retained = 0;
    for item in items.iter_mut().rev() {
        let ContextItem::Checkpoint {
            summary,
            invocation_id,
            ..
        } = item
        else {
            continue;
        };
        if summary["_durable_read_body"] != true || !eligible.contains(invocation_id.as_str()) {
            continue;
        }
        retained += 1;
        if retained <= 2 {
            continue;
        }
        if let Some(content) = summary["content"].as_str() {
            let chars = content.chars().count();
            summary
                .as_object_mut()
                .expect("object read receipt")
                .remove("content");
            summary["read_body_retained_in_journal"] = json!(true);
            summary["read_body_chars"] = json!(chars);
        }
    }
}
