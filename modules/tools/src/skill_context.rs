// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Optional immutable Skill declaration; never inferred from MCP result text.
use crate::Catalog;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Declaration {
    version: u64,
    entry_tools: Vec<String>,
    stages: Vec<Stage>,
    #[serde(default)]
    auto_activate_entry: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Stage {
    tools: Vec<String>,
    omit_sections: Vec<String>,
    #[serde(default)]
    omit_resources: Vec<String>,
}

pub(crate) fn compile(
    catalog: &Catalog,
    id: &str,
    revision: i64,
    text: &str,
    files: &BTreeMap<String, Vec<u8>>,
) -> Option<Value> {
    let bytes = files.get("context-stages.json")?;
    if bytes.len() > 16 * 1024 {
        return None;
    }
    let spec: Declaration = serde_json::from_slice(bytes).ok()?;
    if spec.version != 1 || spec.stages.is_empty() || spec.stages.len() > 16 {
        return None;
    }
    let resolve = |names: &[String]| -> Option<Vec<String>> {
        if names.is_empty() || names.len() > 16 {
            return None;
        }
        let mut resolved = BTreeSet::new();
        for name in names {
            let mut found = catalog
                .bindings
                .values()
                .filter(|b| b.tool.name == *name || b.raw_name == *name);
            let binding = found.next()?;
            if found.next().is_some() || !resolved.insert(binding.tool.name.clone()) {
                return None;
            }
        }
        Some(resolved.into_iter().collect())
    };
    let entry = resolve(&spec.entry_tools)?;
    let mut headings = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.starts_with("## ") {
            headings.push((line.trim_end().to_string(), offset));
        }
        offset += line.len();
    }
    let mut stages = Vec::new();
    let mut workflow = BTreeSet::new();
    for stage in spec.stages {
        let tools = resolve(&stage.tools)?;
        workflow.extend(tools.iter().cloned());
        if stage.omit_sections.len() > 32 {
            return None;
        }
        let mut ranges = BTreeSet::new();
        for heading in stage.omit_sections {
            let matches: Vec<_> = headings
                .iter()
                .enumerate()
                .filter(|(_, h)| h.0 == heading)
                .collect();
            if matches.len() != 1 {
                return None;
            }
            let (index, (_, start)) = matches[0];
            let end = headings.get(index + 1).map_or(text.len(), |h| h.1);
            if !ranges.insert((*start, end)) {
                return None;
            }
        }
        if stage.omit_resources.len() > 32 {
            return None;
        }
        let mut paths = BTreeSet::new();
        let mut resources = Vec::new();
        for path in stage.omit_resources {
            if path == "SKILL.md" || !paths.insert(path.clone()) {
                return None;
            }
            let content = files.get(&path)?;
            std::str::from_utf8(content).ok()?;
            resources.push(
                json!({"path":path,"content_sha256":format!("{:x}",Sha256::digest(content))}),
            );
        }
        stages.push(json!({"tools":tools,"omit_ranges":ranges,"omit_resources":resources}));
    }
    if !entry.iter().all(|tool| workflow.contains(tool)) {
        return None;
    }
    Some(json!({"version":1,"id":id,"revision":revision,
        "content_sha256":format!("{:x}",Sha256::digest(text.as_bytes())),
        "entry_tools":entry,"workflow_tools":workflow,"stages":stages,"auto_activate_entry":spec.auto_activate_entry}))
}
