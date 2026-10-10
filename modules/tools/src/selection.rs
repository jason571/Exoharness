// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Selection identities resolve only against the frozen authorized bindings.
//! Incomplete public labels are correction hints, never executable aliases.
use crate::{Binding, Result, ToolError, public_name};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn resolve(
    bindings: &BTreeMap<String, Binding>,
    names: &[String],
    mode: &str,
) -> Result<BTreeSet<String>> {
    let mut resolved = BTreeSet::new();
    let mut corrections = Vec::new();
    let mut lookups = Vec::new();
    let mut unresolved = Vec::new();
    for name in names {
        if bindings.contains_key(name) {
            resolved.insert(name.clone());
            continue;
        }
        let matches: Vec<_> = bindings
            .values()
            .filter(|b| &b.capability == name || &b.raw_name == name)
            .collect();
        if matches.len() == 1 {
            resolved.insert(matches[0].tool.name.clone());
            continue;
        }
        if !matches.is_empty() {
            lookups.push(if matches.iter().any(|b| &b.capability == name) {
                json!({"query":"","kind":"tool","capability_id":name})
            } else {
                json!({"query":"","kind":"tool","names":[name]})
            });
            continue;
        }
        // Derive labels from authorized identities, not from untrusted input.
        // Keep the hash-based public-name contract unchanged. A stale full ID,
        // typo or arbitrary prefix cannot match this exact hashless label.
        let labels: Vec<_> = bindings
            .values()
            .filter(|b| {
                let canonical = public_name(&b.capability, &b.raw_name);
                canonical == b.tool.name
                    && canonical
                        .rsplit_once('_')
                        .is_some_and(|(label, _)| label == name)
            })
            .collect();
        if labels.is_empty() {
            unresolved.push(name.clone());
            // A logical name from a Skill need not be an installed identity.
            // Discover metadata; never turn a lexical match into permission
            // or silently activate a guessed alias. Respect query's schema.
            lookups.push(json!({
                "query":name.chars().take(200).collect::<String>(),
                "kind":"tool"
            }));
            continue;
        }
        if labels.len() == 1 {
            let binding = labels[0];
            resolved.insert(binding.tool.name.clone());
            corrections.push(json!({"requested":name,"exact_name":binding.tool.name}));
        } else {
            // Sanitization/truncation or multiple providers can share a label.
            // Never select the first match or load all matches implicitly.
            for binding in labels {
                let lookup = json!({"query":"","kind":"tool","names":[binding.raw_name]});
                if !lookups.contains(&lookup) {
                    lookups.push(lookup);
                }
            }
        }
    }
    if !corrections.is_empty() || !lookups.is_empty() {
        let next = if lookups.is_empty() {
            json!({"tool":"activate_tools","arguments":{"names":resolved,"mode":mode}})
        } else {
            serde_json::Value::Null
        };
        let diagnostics = json!({"name_corrections":corrections,
            "resolved_names":resolved,"unresolved_names":unresolved,"mode":mode,
            "required_lookups":lookups,"next_call":next});
        return Err(ToolError::CorrectableInput(format!(
            "Tool selection was rejected before loading or execution; the previous loaded set is unchanged. Skill operation names are instructions, not installed identities. Use exact authorized names, not incomplete labels or service IDs. Correct the ENTIRE proposed batch once using {diagnostics}, retaining its resolved names and mode. If lookups are required, discover the exact identities first; do not guess, widen permission or enumerate unrelated services. If discovery finds no authorized match, report the missing capability instead of repeating the unchanged selection."
        )));
    }
    Ok(resolved)
}
