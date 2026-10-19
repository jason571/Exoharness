// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Host-owned discovery metadata. Never parses instructions or MCP prose.
use exoharness_contracts::model::Tool;
use exoharness_core::context::JournalEntry;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const INDEX_PREFIX: &str = "\nAuthorized capability index (service summaries only; no tool schemas; use next_page unchanged for additional services): ";
const SKILL_PREFIX: &str = "\nAuthorized Skill candidates for this task (metadata only; use read_skill for applicable instructions, not all candidates): ";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitialDiscovery {
    index: Value,
    skills: Vec<Value>,
}

impl InitialDiscovery {
    pub fn new(index: Value, skills: Vec<Value>) -> Self {
        Self { index, skills }
    }

    pub fn system(&self, system: &str, entries: &[JournalEntry], tools: &[Tool]) -> String {
        let mut index = self.index.clone();
        let mut skills = self.skills.clone();
        let start = entries
            .iter()
            .rposition(|e| matches!(e, JournalEntry::User(_)))
            .unwrap_or(0);
        let current = &entries[start..];
        let live: BTreeSet<_> = tools.iter().map(|t| t.name.as_str()).collect();
        let business_loaded = live.iter().any(|name| {
            !matches!(
                *name,
                "search_capabilities" | "read_skill" | "activate_tools"
            )
        });
        let last_failed = current
            .iter()
            .rev()
            .find_map(|e| match e {
                JournalEntry::ToolExchange { result, .. } => Some(failed(result)),
                _ => None,
            })
            .unwrap_or(false);
        let selection = current.iter().any(|e| match e {
            JournalEntry::ToolExchange { tool, result, .. }
                if tool == "activate_tools" && !failed(result) && result["executed"] == false =>
            {
                result["loaded"].as_array().is_some_and(|names| {
                    !names.is_empty()
                        && names
                            .iter()
                            .all(|n| n.as_str().is_some_and(|n| live.contains(n)))
                })
            }
            JournalEntry::ToolExchange {
                tool,
                arguments,
                result,
                ..
            } if tool == "read_skill" => root(arguments, result).is_some(),
            _ => false,
        });
        let valid_index = index["items"].as_array().is_some_and(|items| {
            items.iter().all(|item| {
                item["kind"] == "capability"
                    && item["id"].is_string()
                    && item["name"].is_string()
                    && item["revision"].is_u64()
                    && item["description"].is_string()
            })
        });
        if business_loaded && selection && !last_failed && valid_index {
            // Keep authorized identities visible: a selected authoring Skill
            // may still need another service for evidence or final delivery.
            // Only verbose recognition metadata and per-item navigation defer.
            index["items"] = Value::Array(
                index["items"]
                    .as_array()
                    .expect("validated")
                    .iter()
                    .map(|item| {
                        let mut identity = serde_json::Map::new();
                        for key in ["kind", "id", "name", "revision", "tool_count"] {
                            if let Some(value) = item.get(key) {
                                identity.insert(key.into(), value.clone());
                            }
                        }
                        Value::Object(identity)
                    })
                    .collect(),
            );
            index["catalog_deferred_after_selection"] = json!(true);
            index["catalog_retrieval"] =
                json!({"tool":"search_capabilities","arguments":{"query":"","kind":"capability"}});
            index["note"] = json!(
                "Listed services remain authorized candidates; their descriptions and tools are deferred, not unavailable. Use a listed service ID to discover its tools, or rediscover full summaries when needed. A selected authoring tool does not imply that retrieval or other listed services are absent. Live schemas remain complete."
            );
            let roots: BTreeSet<_> = current
                .iter()
                .filter_map(|e| match e {
                    JournalEntry::ToolExchange {
                        tool,
                        arguments,
                        result,
                        ..
                    } if tool == "read_skill" => root(arguments, result),
                    _ => None,
                })
                .collect();
            for skill in &mut skills {
                let identity = skill["id"].as_str().zip(skill["revision"].as_u64());
                if identity.is_some_and(|identity| roots.contains(&identity))
                    && let Some(object) = skill.as_object_mut()
                {
                    object.remove("description");
                    object.insert("instructions_loaded".into(), json!(true));
                }
            }
            if index.to_string().len() + json!(&skills).to_string().len()
                >= self.index.to_string().len() + json!(&self.skills).to_string().len()
            {
                index = self.index.clone();
                skills = self.skills.clone();
            }
        }
        // Initial/unresolved requests retain the exact legacy text and JSON.
        let mut out = format!("{system}{INDEX_PREFIX}{index}");
        if !skills.is_empty() {
            out.push_str(SKILL_PREFIX);
            out.push_str(&json!(skills).to_string());
        }
        out
    }
}

fn failed(result: &Value) -> bool {
    result["isError"] == true || result["is_error"] == true
}

fn root<'a>(arguments: &Value, result: &'a Value) -> Option<(&'a str, u64)> {
    if failed(result)
        || arguments["path"].as_str().unwrap_or("SKILL.md") != "SKILL.md"
        || !result["content"].is_string()
    {
        return None;
    }
    let id = result["id"].as_str()?;
    (arguments["id"].as_str() == Some(id)).then_some((id, result["revision"].as_u64()?))
}
