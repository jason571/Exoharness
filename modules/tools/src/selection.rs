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
            return Err(ToolError::Input(
                "该名称没有唯一对应的已授权执行工具。Skill 是说明，不能直接激活；请用 search_capabilities 的 kind=tool 查询，使用返回的 name。加载集合未改变；尚未执行".into(),
            ));
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
        let diagnostics =
            json!({"name_corrections":corrections,"required_lookups":lookups,"next_call":next});
        return Err(ToolError::CorrectableInput(format!(
            "Tool selection was rejected before loading or execution; the previous loaded set is unchanged. Use exact authorized names, not incomplete labels or service IDs. Correct the ENTIRE proposed batch once using {diagnostics}. If lookups are required, discover the scoped exact identities first; do not guess, widen permission or enumerate unrelated services."
        )));
    }
    Ok(resolved)
}
