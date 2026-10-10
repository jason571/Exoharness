//! Deterministic, reversible stage view from host-verified journal receipts.
use exoharness_core::context::{ContextItem, JournalEntry};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub struct WorkflowView {
    pub invocation: String,
    pub required: bool,
    pub tools: Vec<String>,
    provider_tools: BTreeSet<String>,
    consumed_invocations: BTreeSet<String>,
}

/// A schema reload disables the compact view, not the underlying obligation.
pub fn workflow_required(entries: &[JournalEntry]) -> bool {
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    let mut scopes = BTreeMap::new();
    for entry in &entries[start..] {
        if let JournalEntry::Checkpoint { summary, .. } = entry {
            let host = &summary["_host_workflow"];
            if host["version"] == 1
                && summary["isError"] != true
                && let Some(scope) = host["capability"].as_str()
                && let Some(status @ ("required" | "complete")) =
                    host["directive"]["status"].as_str()
            {
                scopes.insert(scope, status);
            }
        }
    }
    scopes.values().any(|status| *status == "required")
}

/// External result metadata alone never authorizes projection. Only an adapter
/// checkpoint, reconstructed from a committed exact exchange, is eligible.
pub fn workflow_view(entries: &[JournalEntry]) -> Option<WorkflowView> {
    view(entries, true)
}

// Loading schemas changes the dispatch view, not the durability of an already
// committed checkpoint. Context and schema selection are separate concerns.
fn view(entries: &[JournalEntry], reset_on_activation: bool) -> Option<WorkflowView> {
    let start = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .unwrap_or(0);
    let mut scopes = BTreeMap::<String, (usize, String, Value)>::new();
    let mut consumed = BTreeMap::<String, BTreeSet<String>>::new();
    for (index, entry) in entries.iter().enumerate().skip(start) {
        let JournalEntry::Checkpoint {
            invocation_id,
            summary,
            ..
        } = entry
        else {
            continue;
        };
        let host = &summary["_host_workflow"];
        if summary["isError"] == true || host["version"] != 1 {
            continue;
        }
        let Some(scope) = host["capability"].as_str() else {
            continue;
        };
        let directive = &host["directive"];
        if directive["context_scope"] != "checkpoint"
            || !matches!(directive["status"].as_str(), Some("required" | "complete"))
        {
            continue;
        }
        let tools: Vec<String> = serde_json::from_value(host["tools"].clone()).ok()?;
        consumed.entry(scope.into()).or_default().extend(tools);
        scopes.insert(scope.into(), (index, invocation_id.clone(), host.clone()));
    }
    let pending: Vec<_> = scopes
        .values()
        .filter(|(_, _, h)| h["directive"]["status"] == "required")
        .collect();
    if pending.len() > 1 {
        return None;
    } // Independent unfinished work must survive.
    let (index, invocation, host) = pending
        .first()
        .copied()
        .or_else(|| scopes.values().max_by_key(|v| v.0))?;
    if entries[index + 1..].iter().any(|e| {
        matches!(e, JournalEntry::User(_))
            || (reset_on_activation
                && matches!(e, JournalEntry::ToolExchange{tool,..} if tool == "activate_tools"))
    }) {
        return None;
    }
    let required = host["directive"]["status"] == "required";
    let consumed_tools = consumed
        .remove(host["capability"].as_str()?)
        .unwrap_or_default();
    let provider_tools: BTreeSet<String> =
        serde_json::from_value(host["provider_tools"].clone()).ok()?;
    // A prerequisite name alone does not prove that every call to it belongs
    // to this workflow. Only internal artifacts actually referenced by the
    // committed finishing invocation are retired; independent audio survives.
    let mut references = BTreeSet::new();
    if let Some(arguments) = entries.iter().find_map(|entry| match entry {
        JournalEntry::ToolExchange {
            invocation_id,
            arguments,
            ..
        } if invocation_id == invocation => Some(arguments),
        _ => None,
    }) {
        strings(arguments, &mut references);
    }
    let consumed_invocations = entries[..*index]
        .iter()
        .filter_map(|entry| {
            let JournalEntry::ToolExchange {
                invocation_id,
                tool,
                result,
                ..
            } = entry
            else {
                return None;
            };
            if consumed_tools.contains(tool)
                && result["artifact_visibility"] == "internal"
                && referenced_artifact(result, &references)
            {
                Some(invocation_id.clone())
            } else {
                None
            }
        })
        .collect();
    Some(WorkflowView {
        invocation: invocation.clone(),
        required,
        tools: serde_json::from_value(host["tools"].clone()).ok()?,
        provider_tools,
        consumed_invocations,
    })
}

pub(crate) fn compact(items: &mut Vec<ContextItem>, entries: &[JournalEntry]) {
    let Some(view) = view(entries, false) else {
        return;
    };
    let Some(boundary) = items.iter().position(|item| matches!(item, ContextItem::Checkpoint{invocation_id,..} if invocation_id == &view.invocation)) else { return; };
    let mut position = 0;
    items.retain(|item| {
        let before = position < boundary;
        position += 1;
        if !before {
            return true;
        }
        match item {
            ContextItem::ToolExchange {
                invocation_id,
                tool,
                ..
            } => {
                if tool == "search_capabilities" || tool == "activate_tools" {
                    return false;
                }
                if !view.required && tool == "read_skill" {
                    return false;
                }
                // The required step still needs exact prerequisite receipts;
                // complete means those inputs have been durably consumed.
                !(view.provider_tools.contains(tool)
                    || (!view.required && view.consumed_invocations.contains(invocation_id)))
            }
            ContextItem::Checkpoint {
                invocation_id,
                summary,
                ..
            } => {
                // An earlier finished deliverable is independent evidence,
                // even if a later operation uses the same provider.
                if summary["_host_workflow"]["directive"]["status"] == "complete" {
                    return true;
                }
                let tool = entries.iter().find_map(|entry| match entry {
                    JournalEntry::ToolExchange {
                        invocation_id: id,
                        tool,
                        ..
                    } if id == invocation_id => Some(tool),
                    _ => None,
                });
                !tool.is_some_and(|tool| view.provider_tools.contains(tool))
            }
            ContextItem::Skill { .. } => view.required,
            _ => true,
        }
    });
}

fn strings<'a>(value: &'a Value, out: &mut BTreeSet<&'a str>) {
    match value {
        Value::String(text) => {
            out.insert(text);
        }
        Value::Array(values) => {
            for value in values {
                strings(value, out);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                strings(value, out);
            }
        }
        _ => {}
    }
}

fn referenced_artifact(value: &Value, references: &BTreeSet<&str>) -> bool {
    match value {
        Value::Object(values) => values.iter().any(|(key, value)| {
            (matches!(
                key.as_str(),
                "url" | "download_url" | "artifact_id" | "attachment_id" | "uri"
            ) && value.as_str().is_some_and(|text| references.contains(text)))
                || referenced_artifact(value, references)
        }),
        Value::Array(values) => values
            .iter()
            .any(|value| referenced_artifact(value, references)),
        _ => false,
    }
}
