// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Reversible stage views declared by the authorized immutable Skill package.
use exoharness_core::context::{ContextItem, JournalEntry};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn verified(arguments: &Value, result: &Value) -> Option<(String, Value)> {
    if result["isError"] == true || arguments["path"].as_str().unwrap_or("SKILL.md") != "SKILL.md" {
        return None;
    }
    let text = result["content"].as_str()?;
    let context = &result["_host_skill_context"];
    if context["version"] != 1
        || context["id"] != arguments["id"]
        || context["id"] != result["id"]
        || context["revision"] != result["revision"]
        || context["content_sha256"] != format!("{:x}", Sha256::digest(text.as_bytes()))
    {
        return None;
    }
    Some((text.into(), context.clone()))
}

pub fn initial_skill_tools(entries: &[JournalEntry]) -> Option<(Vec<String>, Vec<String>)> {
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    let mut context: Option<Value> = None;
    let mut reads = BTreeSet::new();
    for entry in &entries[start..] {
        if let JournalEntry::ToolExchange {
            tool,
            arguments,
            result,
            ..
        } = entry
        {
            if !matches!(
                tool.as_str(),
                "read_skill" | "search_capabilities" | "activate_tools"
            ) {
                return None;
            }
            if tool == "read_skill" {
                if arguments["path"].as_str().unwrap_or("SKILL.md") != "SKILL.md" {
                    let root = context.as_ref()?;
                    if arguments["id"] != root["id"]
                        || result["id"] != root["id"]
                        || result["revision"] != root["revision"]
                        || result["isError"] == true
                        || (!result["content"].is_string() && result["already_loaded"] != true)
                    {
                        return None;
                    }
                    continue;
                }
                if result["already_loaded"] == true {
                    continue;
                }
                if !reads.insert(arguments["id"].as_str()?.to_string()) {
                    return None;
                }
                let (_, value) = verified(arguments, result)?;
                if context.replace(value).is_some() {
                    return None;
                }
            }
        }
    }
    let context = context?;
    Some((
        serde_json::from_value(context["entry_tools"].clone()).ok()?,
        serde_json::from_value(context["workflow_tools"].clone()).ok()?,
    ))
}

pub(crate) fn project(items: &mut [ContextItem], entries: &[JournalEntry], names: &[String]) {
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    let current: BTreeSet<_> = entries[start..]
        .iter()
        .filter_map(|e| match e {
            JournalEntry::ToolExchange {
                invocation_id,
                tool,
                ..
            } if tool == "read_skill" => Some(invocation_id.as_str()),
            _ => None,
        })
        .collect();
    let names: BTreeSet<_> = names
        .iter()
        .filter(|n| {
            !matches!(
                n.as_str(),
                "read_skill" | "search_capabilities" | "activate_tools"
            )
        })
        .cloned()
        .collect();
    if names.is_empty() {
        return;
    }
    let mut resources = Vec::new();
    for item in items.iter_mut() {
        let ContextItem::ToolExchange {
            invocation_id,
            tool,
            arguments,
            result,
        } = item
        else {
            continue;
        };
        if tool != "read_skill" || !current.contains(invocation_id.as_str()) {
            continue;
        }
        let Some((text, context)) = verified(arguments, result) else {
            continue;
        };
        let Some(stages) = context["stages"].as_array() else {
            continue;
        };
        let candidates: Vec<_> = stages
            .iter()
            .filter(|stage| {
                serde_json::from_value::<BTreeSet<String>>(stage["tools"].clone())
                    .is_ok_and(|tools| names.is_subset(&tools))
            })
            .collect();
        if candidates.len() != 1 {
            continue;
        }
        let Ok(ranges) =
            serde_json::from_value::<Vec<(usize, usize)>>(candidates[0]["omit_ranges"].clone())
        else {
            continue;
        };
        // Selecting later tools is not evidence that their prerequisites ran.
        // Omit instructions only after a committed workflow receipt belonging
        // to this Skill permits the selected tools. Schema reloads alone must
        // never erase preparation or repair instructions.
        let omitted = candidates[0]["omit_resources"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if (!ranges.is_empty() || !omitted.is_empty())
            && !stage_committed(entries, invocation_id, &context, &names)
        {
            continue;
        }
        let mut cursor = 0;
        let mut projected = String::new();
        let mut valid = true;
        for (start, end) in ranges {
            if start < cursor
                || end <= start
                || end > text.len()
                || !text.is_char_boundary(start)
                || !text.is_char_boundary(end)
            {
                valid = false;
                break;
            }
            projected.push_str(&text[cursor..start]);
            cursor = end;
        }
        if !valid {
            continue;
        }
        projected.push_str(&text[cursor..]);
        result["content"] = Value::String(projected);
        result["skill_context_projection"] = serde_json::json!({"source_sha256":context["content_sha256"],"stage_tools":names,"full_instructions_retained_in_journal":true});
        if let Some(object) = result.as_object_mut() {
            object.remove("_host_skill_context");
        }
        if !omitted.is_empty() {
            resources.push((context, omitted));
        }
    }
    super::skill_resources::project(items, entries, &resources);
}

fn stage_committed(
    entries: &[JournalEntry],
    read: &str,
    context: &Value,
    names: &BTreeSet<String>,
) -> bool {
    let Some(view) = super::workflow::context_view(entries) else {
        return false;
    };
    let Ok(workflow_tools) =
        serde_json::from_value::<BTreeSet<String>>(context["workflow_tools"].clone())
    else {
        return false;
    };
    if !names.is_subset(&view.tools.into_iter().collect()) {
        return false;
    }
    let read_index = entries.iter().position(|entry| {
        matches!(entry, JournalEntry::ToolExchange { invocation_id, tool, .. }
            if invocation_id == read && tool == "read_skill")
    });
    let receipt = entries
        .iter()
        .enumerate()
        .find_map(|(index, entry)| match entry {
            JournalEntry::ToolExchange {
                invocation_id,
                tool,
                result,
                ..
            } if invocation_id == &view.invocation
                && workflow_tools.contains(tool)
                && result["isError"] != true =>
            {
                Some((index, tool))
            }
            _ => None,
        });
    let (Some(read), Some((receipt, tool))) = (read_index, receipt) else {
        return false;
    };
    if receipt <= read {
        return false;
    }
    // Shared tools cannot attribute one receipt to two different Skill roots.
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    !entries[start..].iter().any(|entry| {
        let JournalEntry::ToolExchange {
            tool: name,
            arguments,
            result,
            ..
        } = entry
        else {
            return false;
        };
        if name != "read_skill" {
            return false;
        }
        let Some((_, other)) = verified(arguments, result) else {
            return false;
        };
        (other["id"] != context["id"] || other["revision"] != context["revision"])
            && other["workflow_tools"]
                .as_array()
                .is_some_and(|tools| tools.iter().any(|name| name == tool))
    })
}
