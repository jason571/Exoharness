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
    let mut context = None;
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
    }
}
