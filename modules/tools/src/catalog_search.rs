// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Two-level authorized discovery. Metadata selection precedes schema loading.
use crate::{Catalog, Result, ToolError};
use exoharness_contracts::capability::CapabilitySpec;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const PAGE: usize = 12;

fn normalized(value: &str) -> String {
    value
        .to_lowercase()
        .replace(['_', '-', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn score(query: &str, name: &str, description: &str) -> usize {
    let words = crate::discovery::terms(query);
    if words.is_empty() {
        return 1;
    }
    let exact = normalized(query) == normalized(name);
    let name = name.to_lowercase();
    let description = description.to_lowercase();
    words
        .iter()
        .map(|w| {
            if name.contains(w) {
                4
            } else if description.contains(w) {
                1
            } else {
                0
            }
        })
        .sum::<usize>()
        + if exact { 1_000_000 } else { 0 }
}
fn ranked(mut values: Vec<(usize, Value)>) -> Vec<Value> {
    values.retain(|(score, _)| *score > 0);
    values.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    values.into_iter().map(|(_, value)| value).collect()
}

impl Catalog {
    /// No separate routing model request. The Agent selects from service
    /// metadata and only then asks for a scoped tool listing or exact names.
    pub fn capability_index(&mut self) -> Value {
        self.discover(&json!({"query":"","kind":"capability"}))
            .expect("well-formed unscoped native discovery")
    }

    fn providers(&self, query: &str) -> Vec<Value> {
        let mut groups: BTreeMap<&str, Vec<&crate::Binding>> = BTreeMap::new();
        for binding in self.bindings.values() {
            groups.entry(&binding.capability).or_default().push(binding);
        }
        ranked(
            groups
                .into_iter()
                .map(|(id, tools)| {
                    let record = self.records.get(id);
                    let name = record.map_or(id, |r| r.manifest.name.as_str());
                    let description = record
                        .map(|r| r.manifest.description.clone())
                        .unwrap_or_else(|| {
                            tools
                                .iter()
                                .take(2)
                                .map(|b| b.tool.description.as_str())
                                .collect::<Vec<_>>()
                                .join("; ")
                        });
                    let relevance = score(query, name, &description).max(
                        tools
                            .iter()
                            .map(|b| score(query, &b.raw_name, &b.tool.description))
                            .max()
                            .unwrap_or(0),
                    );
                    (
                        relevance,
                        json!({"kind":"capability","id":id,"name":name,
                "description":description.chars().take(320).collect::<String>(),
                "revision":record.map_or(tools[0].revision, |r| r.revision as u64),
                "tool_count":tools.len(),"id_usage":"Discovery scope only; not an activation name",
                "discover":{"tool":"search_capabilities","arguments":{"query":"","kind":"tool","capability_id":id}}}),
                    )
                })
                .collect(),
        )
    }

    pub(crate) fn search_scoped(
        &self,
        query: &str,
        offset: usize,
        kind: Option<&str>,
        scope: Option<&str>,
        names: &[String],
    ) -> Value {
        let results = if kind == Some("capability") {
            self.providers(query)
        } else {
            let tools = self.bindings.values().filter(|b| scope.is_none_or(|id| id == b.capability))
                .filter(|b| names.is_empty() || names.iter().any(|name| name == &b.raw_name || name == &b.tool.name))
                .map(|b| {
                    let exact = !query.trim().is_empty() && [&b.raw_name, &b.tool.name, &b.capability].iter()
                        .any(|name| normalized(name) == normalized(query));
                    (if names.is_empty() { score(query, &format!("{} {} {}", b.capability, b.raw_name, b.tool.name), &b.tool.description) + if exact {1_000_000} else {0} } else {1},
                     json!({"kind":"tool","name":b.tool.name,"raw_name":b.raw_name,
                        "description":b.tool.description.chars().take(320).collect::<String>(),
                        "capability_id":b.capability,"revision":b.revision,"schema_loaded":self.active.contains(&b.tool.name)}))
                });
            let skills = self.records.values().filter(|r| matches!(r.manifest.spec, CapabilitySpec::Skill(_)))
                .filter(|_| scope.is_none() && names.is_empty())
                .map(|r| (score(query, &r.manifest.name, &r.manifest.description), json!({"kind":"skill","id":r.id,"name":r.manifest.name,"revision":r.revision,"description":r.manifest.description.chars().take(480).collect::<String>()})));
            ranked(
                tools
                    .chain(skills)
                    .filter(|(_, item)| kind.is_none_or(|kind| item["kind"] == kind))
                    .collect(),
            )
        };
        let related_skills = if kind == Some("tool") && !query.trim().is_empty() {
            self.skill_candidates(query)
        } else {
            vec![]
        };
        json!({"items":results.iter().skip(offset).take(PAGE).collect::<Vec<_>>(),"total":results.len(),
            "next_offset":(offset.saturating_add(PAGE)<results.len()).then(||offset.saturating_add(PAGE)),
            "related_skills":related_skills,
            "tool_discovery":{"tool":"search_capabilities","arguments":{"query":"","kind":"capability"}},
            "note":"Select a relevant capability, then discover only its tools or batch exact names from the Skill. Search metadata does not load schemas, execute work or grant permissions. A lexical miss is not proof of global unavailability."})
    }

    /// Cursor is bound to query, scope, name set and frozen revisions. Legacy
    /// offsets remain supported within the same search, but never cross queries.
    pub fn discover(&mut self, arguments: &Value) -> Result<Value> {
        if !arguments.is_object() {
            return Err(ToolError::Input(
                "查询参数必须为对象；未改变加载集合".into(),
            ));
        }
        let query = arguments["query"].as_str().unwrap_or("");
        let scope = arguments["capability_id"].as_str();
        let requested_kind = arguments["kind"].as_str();
        if scope.is_some() && matches!(requested_kind, Some("skill" | "capability")) {
            return Err(ToolError::CorrectableInput(
                "capability_id scopes executable tools only: use kind=tool with an exact service ID from the authorized index. To find Skill instructions, use kind=skill without capability_id, then read_skill with a returned exact ID. To find services, use kind=capability without capability_id. No capability was executed or permissions changed.".into(),
            ));
        }
        if scope.is_some_and(|id| !self.bindings.values().any(|b| b.capability == id)) {
            // Discovery sees only the frozen authorized index. An absent ID
            // may be invented, stale or inaccessible; it proves no denial.
            // Return the same bounded correction without revealing existence.
            return Err(ToolError::CorrectableInput(
                "The supplied capability_id is not an executable service in the authorized index. Remove capability_id and use kind=capability to find available services, or kind=skill to find instructions; then use only exact returned identities. Do not repeat this scope or infer that the requested task lacks permission. No capability was executed or permissions changed.".into(),
            ));
        }
        let names: Vec<String> = arguments
            .get("names")
            .map(|v| serde_json::from_value(v.clone()))
            .transpose()
            .map_err(|_| ToolError::Input("names 必须为工具名称数组；未改变加载集合".into()))?
            .unwrap_or_default();
        let kind = if requested_kind == Some("skill") || requested_kind == Some("capability") {
            requested_kind
        } else if query.trim().is_empty() && scope.is_none() && names.is_empty() {
            Some("capability")
        } else if !names.is_empty() || scope.is_some() {
            Some("tool")
        } else {
            requested_kind
        };
        if !names.is_empty() && kind != Some("tool") {
            return Err(ToolError::Input(
                "names 查询用于执行工具；请使用 kind=tool".into(),
            ));
        }
        let mut sorted_names = names.clone();
        sorted_names.sort();
        let revisions: Vec<_> = self
            .bindings
            .values()
            .map(|b| (&b.tool.name, b.revision))
            .collect();
        let skill_revisions: Vec<_> = self.records.values().map(|r| (&r.id, r.revision)).collect();
        let key = format!(
            "{:x}",
            Sha256::digest(
                json!([query, kind, scope, sorted_names, revisions, skill_revisions])
                    .to_string()
                    .as_bytes()
            )
        );
        let requested_offset = arguments["offset"].as_u64().unwrap_or(0) as usize;
        let cursor_offset = arguments["cursor"].as_str().and_then(|cursor| {
            let (digest, offset) = cursor.split_once(':')?;
            (digest == key)
                .then(|| offset.parse::<usize>().ok())
                .flatten()
        });
        let reset = if arguments.get("cursor").is_some() {
            cursor_offset.is_none()
        } else {
            requested_offset > 0 && self.discovery_key.as_deref() != Some(&key)
        };
        let offset = if reset {
            0
        } else {
            cursor_offset.unwrap_or(requested_offset)
        };
        self.discovery_key = Some(key.clone());
        let mut result = self.search_scoped(query, offset, kind, scope, &names);
        // A batch name lookup resolves every requested name, including ambiguity,
        // without dropping later matches behind the normal description page.
        if !names.is_empty() {
            let mut items = Vec::new();
            let mut missing = Vec::new();
            for name in &names {
                let matches: Vec<_> = self
                    .bindings
                    .values()
                    .filter(|b| scope.is_none_or(|id| id == b.capability))
                    .filter(|b| name == &b.raw_name || name == &b.tool.name)
                    .collect();
                if matches.is_empty() {
                    missing.push(name);
                }
                for binding in matches {
                    if !items.iter().any(|v: &Value| v["name"] == binding.tool.name) {
                        items.push(json!({"kind":"tool","name":binding.tool.name,"raw_name":binding.raw_name,
                            "capability_id":binding.capability,"revision":binding.revision,
                            "schema_loaded":self.active.contains(&binding.tool.name)}));
                    }
                }
            }
            result["items"] = json!(items);
            result["total"] = json!(items.len());
            result["missing_names"] = json!(missing);
            result["next_offset"] = Value::Null;
        }
        result["kind"] = json!(kind);
        result["offset"] = json!(offset);
        result["offset_reset"] = json!(reset);
        result["next_page"] = result["next_offset"]
            .as_u64()
            .map(|next| {
                let mut next_arguments = arguments.clone();
                let object = next_arguments
                    .as_object_mut()
                    .expect("validated native arguments");
                object.remove("offset");
                if let Some(kind) = kind {
                    object.insert("kind".into(), json!(kind));
                } else {
                    object.remove("kind");
                }
                object.insert("cursor".into(), json!(format!("{key}:{next}")));
                next_arguments
            })
            .unwrap_or(Value::Null);
        Ok(result)
    }
}
