// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only

//! Deterministic retrieval inside one already authorized immutable Skill package.
use crate::{Result, ToolError};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const INDEX: &str = "resource-index.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    version: u32,
    resources: Vec<Resource>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resource {
    id: String,
    path: String,
    terms: Vec<String>,
    sha256: String,
}

fn invalid() -> ToolError {
    ToolError::CorrectableInput("This Skill's optional resource index is invalid; no supplementary resources were loaded. Read SKILL.md and use its primary workflow if sufficient. Do not retry the unchanged index or infer missing task permissions. No execution was submitted.".into())
}

fn matches(query: &str, term: &str) -> bool {
    query.match_indices(term).any(|(start, _)| {
        // ASCII words must not match inside unrelated words; CJK phrases match literally.
        if term.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ') {
            !query[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric())
                && !query[start + term.len()..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric())
        } else {
            true
        }
    })
}

pub(crate) fn select(files: &BTreeMap<String, Vec<u8>>, query: &str) -> Result<Value> {
    if query.trim().is_empty() || query.chars().count() > 2000 {
        return Err(ToolError::CorrectableInput(
            "Resource query must contain 1..2000 characters".into(),
        ));
    }
    let bytes = files
        .get(INDEX)
        .filter(|b| b.len() <= 16 * 1024)
        .ok_or_else(invalid)?;
    let index: Index = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if index.version != 1 || index.resources.len() > 32 {
        return Err(invalid());
    }
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let query = query.to_lowercase();
    let mut ranked = Vec::new();
    for resource in index.resources {
        if resource.id.is_empty()
            || resource.id.len() > 100
            || !ids.insert(resource.id.clone())
            || !resource.path.starts_with("references/")
            || !resource.path.ends_with(".md")
            || resource.path.len() > 200
            || resource
                .path
                .split('/')
                .any(|p| p.is_empty() || p == "." || p == "..")
            || resource.path.contains('\\')
            || !paths.insert(resource.path.clone())
            || resource.terms.is_empty()
            || resource.terms.len() > 8
        {
            return Err(invalid());
        }
        let mut terms = BTreeSet::new();
        for term in &resource.terms {
            if !(2..=40).contains(&term.chars().count())
                || term.trim() != term
                || term.to_lowercase() != *term
                || !terms.insert(term)
                || term.chars().any(char::is_control)
            {
                return Err(invalid());
            }
        }
        let content = files
            .get(&resource.path)
            .filter(|b| b.len() <= 8192)
            .ok_or_else(invalid)?;
        if format!("{:x}", Sha256::digest(content)) != resource.sha256
            || std::str::from_utf8(content).is_err()
        {
            return Err(invalid());
        }
        let score = resource.terms.iter().filter(|t| matches(&query, t)).count();
        if score > 0 {
            ranked.push((score, resource));
        }
    }
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    let mut resources = Vec::new();
    let mut total = 0;
    for (_, resource) in ranked {
        let content = &files[&resource.path];
        if resources.len() == 2 || total + content.len() > 8192 {
            continue;
        }
        total += content.len();
        resources.push(
            json!({"id":resource.id,"path":resource.path,"sha256":resource.sha256,
            "content":std::str::from_utf8(content).map_err(|_| invalid())?}),
        );
    }
    Ok(json!({"version":1,"resources":resources,"max_resources":2,
        "message":"Selected package resources only. No match means no learned procedure applies. Current authorization, live schemas and verified receipts remain authoritative."}))
}

#[cfg(test)]
#[path = "../tests/unit/skill_resources.rs"]
mod tests;
