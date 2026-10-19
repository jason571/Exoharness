// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bounded native discovery metadata, never authored inputs or business results.
use exoharness_core::context::{ContextItem, JournalEntry};
use serde_json::json;
use std::collections::BTreeSet;

pub(super) fn compact_selected_descriptions(items: &mut [ContextItem], entries: &[JournalEntry]) {
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    let mut loaded = BTreeSet::new();
    let mut retired = BTreeSet::new();
    let mut selection_completed = false;
    let read_skills: BTreeSet<_> = items
        .iter()
        .filter_map(|item| {
            let ContextItem::ToolExchange {
                tool,
                arguments,
                result,
                ..
            } = item
            else {
                return None;
            };
            if tool != "read_skill"
                || result["isError"] == true
                || result["is_error"] == true
                || arguments["path"].as_str().unwrap_or("SKILL.md") != "SKILL.md"
                || result["content"].as_str().is_none()
            {
                return None;
            }
            let id = result["id"].as_str()?;
            if arguments["id"].as_str() != Some(id) {
                return None;
            }
            Some((id.to_owned(), result["revision"].as_u64()?))
        })
        .collect();
    // A later successful activation proves selection; search alone does not.
    // Walk backwards so a failed activation or a prior turn cannot retire data.
    for entry in entries[start..].iter().rev() {
        let JournalEntry::ToolExchange {
            invocation_id,
            tool,
            result,
            ..
        } = entry
        else {
            continue;
        };
        if result["isError"] == true || result["is_error"] == true {
            continue;
        }
        let selection = if tool == "activate_tools" {
            result
        } else if tool == "read_skill" {
            &result["tool_selection"]
        } else {
            &serde_json::Value::Null
        };
        if selection["executed"] == false {
            if let Some(names) = selection["loaded"].as_array() {
                loaded.extend(names.iter().filter_map(|n| n.as_str()));
                selection_completed |= !names.is_empty();
            }
        } else if tool == "search_capabilities" && selection_completed && result["items"].is_array()
        {
            retired.insert(invocation_id.as_str());
        }
    }
    for item in items {
        let ContextItem::ToolExchange {
            invocation_id,
            tool,
            result,
            ..
        } = item
        else {
            continue;
        };
        if tool != "search_capabilities" || result["isError"] == true || result["is_error"] == true
        {
            continue;
        }
        if retired.contains(invocation_id.as_str()) && result["items"].is_array() {
            let selected: Vec<_> = result["items"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|value| {
                    value["kind"] == "tool"
                        && value["name"].as_str().is_some_and(|n| loaded.contains(n))
                })
                .map(|value| {
                    let mut value = value.clone();
                    if let Some(object) = value.as_object_mut() {
                        object.remove("description");
                    }
                    value
                })
                .collect();
            *result = json!({"items":selected,"catalog_selection_retired":true,
                "catalog_retrieval":{"tool":"search_capabilities","arguments":{"query":"","kind":"capability"}},
                "note":"Historical discovery was consumed by a later successful tool selection. Full pages remain in the journal. Search by capability_id or exact names if more tools are needed; selected live schemas remain authoritative."});
            continue;
        }
        let Some(values) = result["items"].as_array_mut() else {
            continue;
        };
        for value in values {
            // Names, revisions, pagination and unread Skill summaries remain.
            // The active tool's description and complete schema are supplied
            // by the catalog; no substring is cut from a native JSON value.
            if retired.contains(invocation_id.as_str())
                && value["kind"] == "tool"
                && let Some(object) = value.as_object_mut()
            {
                object.remove("description");
            } else {
                summarize(value, &read_skills);
            }
        }
        if let Some(skills) = result["related_skills"].as_array_mut() {
            for skill in skills {
                summarize(skill, &read_skills);
            }
        }
        result["catalog_descriptions_retired"] = json!(retired.contains(invocation_id.as_str()));
        result["catalog_retrieval"] = json!(
            "Discovery summaries only, not parameter constraints. Activation supplies full descriptions and live schemas. Exact discovery remains in the execution journal; read an applicable Skill for its full instructions."
        );
    }
}

fn summarize(value: &mut serde_json::Value, read_skills: &BTreeSet<(String, u64)>) {
    if value["id"]
        .as_str()
        .zip(value["revision"].as_u64())
        .is_some_and(|(id, revision)| read_skills.contains(&(id.into(), revision)))
    {
        if let Some(object) = value.as_object_mut() {
            object.remove("description");
        }
        value["instructions_loaded"] = json!(true);
    } else if let Some(description) = value["description"].as_str() {
        // This is the native catalogue's recognition label, not an arbitrary
        // MCP result or authored JSON. Names, versions and paging stay exact.
        if description.chars().count() > 160 {
            value["description"] = json!(description.chars().take(160).collect::<String>());
            value["description_is_summary"] = json!(true);
        }
    }
}
