// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Immutable references retired only by an already verified Skill stage.
use exoharness_core::context::{ContextItem, JournalEntry};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn project(
    items: &mut [ContextItem],
    entries: &[JournalEntry],
    policies: &[(Value, Vec<Value>)],
) {
    if policies.is_empty() {
        return;
    }
    let Some(view) = super::workflow::context_view(entries) else {
        return;
    };
    let Some(boundary) = entries.iter().position(|entry| {
        matches!(entry,
        JournalEntry::Checkpoint { invocation_id, .. } if invocation_id == &view.invocation)
    }) else {
        return;
    };
    for item in items {
        let ContextItem::ToolExchange {
            invocation_id,
            tool,
            arguments,
            result,
        } = item
        else {
            continue;
        };
        if tool != "read_skill" || result["isError"] == true {
            continue;
        }
        let Some(path) = arguments["path"].as_str() else {
            continue;
        };
        if path == "SKILL.md" || result["path"] != path {
            continue;
        }
        let Some(index) = entries.iter().position(|entry| {
            matches!(entry,
            JournalEntry::ToolExchange { invocation_id: id, .. } if id == invocation_id)
        }) else {
            continue;
        };
        if index >= boundary {
            continue;
        }
        // An explicit later read restores the original reference, including
        // when the dispatch cache reports that it was already loaded.
        if entries[boundary + 1..].iter().any(|entry| {
            matches!(entry,
            JournalEntry::ToolExchange { tool, arguments: args, .. }
                if tool == "read_skill" && args["id"] == arguments["id"] && args["path"] == path)
        }) {
            continue;
        }
        let Some(text) = result["content"].as_str() else {
            continue;
        };
        let digest = format!("{:x}", Sha256::digest(text.as_bytes()));
        let matched = policies.iter().any(|(context, resources)| {
            arguments["id"] == context["id"]
                && result["id"] == context["id"]
                && result["revision"] == context["revision"]
                && resources.iter().any(|resource| {
                    resource["path"] == path && resource["content_sha256"] == digest
                })
        });
        if !matched {
            continue;
        }
        result["content"] = json!(
            "Reference retained in the immutable Skill revision and journal. Omitted for this verified workflow stage. Read this Skill path explicitly if needed again."
        );
        result["skill_resource_projection"] = json!({"source_sha256":digest,
            "full_instructions_retained_in_journal":true});
        if let Some(object) = result.as_object_mut() {
            object.remove("files");
        }
    }
}
