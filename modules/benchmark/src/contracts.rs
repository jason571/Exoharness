// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Result, ensure, list, number, text};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub fn sha256(raw: &[u8]) -> String {
    format!("{:x}", Sha256::digest(raw))
}
pub fn digest(value: &Value) -> String {
    sha256(crate::canonical::json(value).as_bytes())
}
pub fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 80
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}
pub fn hash(value: &Value) -> bool {
    value.as_str().is_some_and(|s| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}
pub fn fields(value: &Value, allowed: &[&str], exact: bool) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| crate::Error::Invalid("Expected an object".into()))?;
    ensure(
        object.keys().all(|k| allowed.contains(&k.as_str()))
            && (!exact || object.len() == allowed.len()),
        "Unexpected or missing contract fields",
    )
}
#[derive(Clone, Debug)]
pub struct Suite(Value);
impl Suite {
    pub fn parse(value: Value) -> Result<Self> {
        ensure(
            value["version"] == 1 && identifier(text(&value["id"])?),
            "Invalid suite identity/version",
        )?;
        let tasks = list(&value["tasks"])?;
        ensure(
            !tasks.is_empty() && tasks.len() <= 1000,
            "Suite requires 1..1000 tasks",
        )?;
        let (mut ids, mut prompts, mut groups, mut combos) = (
            BTreeSet::new(),
            BTreeSet::new(),
            BTreeMap::<String, BTreeSet<String>>::new(),
            BTreeMap::<String, BTreeMap<String, Vec<BTreeSet<String>>>>::new(),
        );
        for task in tasks {
            let id = text(&task["id"])?;
            let group = text(&task["group"])?;
            let split = text(&task["split"])?;
            ensure(
                identifier(id)
                    && ids.insert(id.to_owned())
                    && identifier(group)
                    && ["train", "validation", "test", "regression"].contains(&split),
                "Invalid task identity/split",
            )?;
            let turns = list(&task["turns"])?;
            ensure(
                !turns.is_empty() && turns.len() <= 8,
                "Tasks need 1..8 turns",
            )?;
            let mut normalized = Vec::new();
            for turn in turns {
                let s = text(turn)?;
                ensure(
                    !s.trim().is_empty() && s.chars().count() <= 16000,
                    "Invalid task prompt",
                )?;
                normalized.push(
                    s.split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase()
                        .replace('ß', "ss"),
                );
            }
            ensure(
                prompts.insert(digest(&json!(normalized))),
                "Duplicate task input",
            )?;
            if let Some(e) = task.get("evidence") {
                fields(
                    e,
                    &[
                        "input_sha256",
                        "source_trace_sha256",
                        "state",
                        "verified_delivery",
                        "failure_classes",
                    ],
                    true,
                )?;
                ensure(
                    split == "train"
                        && e["input_sha256"] == digest(&task["turns"])
                        && hash(&e["source_trace_sha256"])
                        && e["verified_delivery"].is_boolean()
                        && ["succeeded", "failed", "recoverable", "cancelled"]
                            .contains(&text(&e["state"])?),
                    "Invalid reviewed training evidence",
                )?;
                for c in list(&e["failure_classes"])? {
                    ensure(
                        [
                            "input_invalid",
                            "transport",
                            "unknown_submission",
                            "capability_denied",
                            "budget",
                            "delivery_invalid",
                        ]
                        .contains(&text(c)?),
                        "Unknown failure class",
                    )?;
                }
            }
            if let Some(v) = task.get("review_required") {
                ensure(v.is_boolean(), "Review flag must be boolean")?;
            }
            for a in task
                .get("attachments")
                .map(list)
                .transpose()?
                .into_iter()
                .flatten()
            {
                let path = text(&a["path"])?;
                ensure(
                    ["audio", "image", "file"].contains(&text(&a["type"])?)
                        && path.starts_with("inputs/")
                        && path[7..]
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
                        && path.len() > 7
                        && hash(&a["sha256"]),
                    "Invalid pinned input",
                )?;
            }
            let checks = list(&task["checks"])?;
            ensure(!checks.is_empty(), "Delivery checks are required")?;
            for c in checks {
                validate_check(c)?;
            }
            groups.entry(group.into()).or_default().insert(split.into());
            if let Some(ruleids) = task.get("rule_ids") {
                let selected = list(ruleids)?;
                let declared = list(&value["rules"][group])?;
                let mut set = BTreeSet::new();
                ensure(!selected.is_empty(), "Empty rule combination")?;
                for r in selected {
                    let name = text(r)?;
                    ensure(
                        identifier(name) && declared.contains(r) && set.insert(name.to_string()),
                        "Invalid rule combination",
                    )?;
                }
                combos
                    .entry(group.into())
                    .or_default()
                    .entry(split.into())
                    .or_default()
                    .push(set);
            }
        }
        for splits in groups.values() {
            ensure(
                splits.contains("train") && splits.contains("test"),
                "Each group needs separate train/test tasks",
            )?;
        }
        for parts in combos.values() {
            let train = parts.get("train").cloned().unwrap_or_default();
            let known: BTreeSet<_> = train.iter().flatten().cloned().collect();
            for split in ["validation", "test"] {
                for combination in parts.get(split).into_iter().flatten() {
                    ensure(
                        !train.contains(combination) && combination.is_subset(&known),
                        "Held-out combinations must recombine training rules",
                    )?;
                }
            }
        }
        Ok(Self(value))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    pub fn sha256(&self) -> String {
        digest(&self.0)
    }
    pub fn tasks(&self) -> &[Value] {
        self.0["tasks"].as_array().expect("validated Suite")
    }
    pub fn training_material(&self) -> Value {
        json!(self.tasks().iter().filter(|t|t["split"]=="train").map(|t|{
            let inputs:Vec<_>=t["attachments"].as_array().into_iter().flatten().map(|a|json!({"type":a["type"],"path":a["path"],"sha256":a["sha256"]})).collect();
            json!({"id":t["id"],"group":t["group"],"turns":t["turns"],"expected_delivery":t["checks"],"reviewed_evidence":t.get("evidence"),"inputs":inputs})
        }).collect::<Vec<_>>())
    }
}
fn strings(value: &Value, nonempty: bool) -> Result<()> {
    let values = list(value)?;
    ensure(
        !nonempty || !values.is_empty(),
        "Expected nonempty string list",
    )?;
    for s in values {
        ensure(!text(s)?.is_empty(), "Empty expected text")?;
    }
    Ok(())
}
fn validate_check(c: &Value) -> Result<()> {
    match text(&c["kind"])? {
        "json_equals" => {
            fields(c, &["kind", "fields"], true)?;
            ensure(
                c["fields"].as_object().is_some_and(|v| !v.is_empty()),
                "Invalid JSON check",
            )?;
        }
        "text_contains" => {
            fields(c, &["kind", "values"], true)?;
            strings(&c["values"], true)?;
        }
        "tool_count" => {
            fields(
                c,
                &["kind", "name_contains", "max", "min", "successful_only"],
                false,
            )?;
            ensure(
                !text(&c["name_contains"])?.is_empty()
                    && number(c.get("min").unwrap_or(&json!(0)))? <= number(&c["max"])?
                    && c.get("successful_only").is_none_or(Value::is_boolean),
                "Invalid tool count check",
            )?;
        }
        "artifact" => {
            fields(
                c,
                &[
                    "kind",
                    "extension",
                    "min_duration",
                    "max_duration",
                    "slides",
                    "audio",
                    "contains",
                ],
                false,
            )?;
            ensure(
                [".html", ".zip", ".pptx", ".mp4", ".mp3", ".wav"]
                    .contains(&text(&c["extension"])?),
                "Unsupported artifact type",
            )?;
            for key in ["min_duration", "max_duration"] {
                if let Some(v) = c.get(key) {
                    ensure(
                        v.as_f64().is_some_and(|n| n.is_finite() && n > 0.0),
                        "Invalid duration",
                    )?;
                }
            }
            ensure(
                c["min_duration"].as_f64().unwrap_or(0.0)
                    <= c["max_duration"].as_f64().unwrap_or(f64::INFINITY),
                "Reversed duration bounds",
            )?;
            if let Some(v) = c.get("slides") {
                ensure(number(v)? > 0, "Invalid slide count")?;
            }
            if let Some(v) = c.get("audio") {
                ensure(v.is_boolean(), "Invalid audio requirement")?;
            }
            if let Some(v) = c.get("contains") {
                strings(v, false)?;
            }
        }
        _ => return Err(crate::Error::Invalid("Unknown delivery check".into())),
    }
    Ok(())
}
#[derive(Clone, Debug)]
pub struct Experience(Value);
impl Experience {
    pub fn parse(value: Value, suite: &Suite) -> Result<Self> {
        ensure(
            value["version"] == 1 && value["suite_sha256"] == suite.sha256(),
            "Candidate does not match suite",
        )?;
        let rules = list(&value["rules"])?;
        ensure(
            !rules.is_empty() && rules.len() <= 12,
            "Candidate requires 1..12 rules",
        )?;
        for r in rules {
            ensure(identifier(text(&r["group"])?), "Invalid rule group")?;
            let sources = list(&r["sources"])?;
            ensure(!sources.is_empty(), "Rule needs training sources")?;
            for s in sources {
                ensure(
                    suite.tasks().iter().any(|t| {
                        t["id"] == *s && t["split"] == "train" && t["group"] == r["group"]
                    }),
                    "Rule source is not matching training data",
                )?;
            }
            for k in ["when", "action", "unless"] {
                let t = text(&r[k])?;
                ensure(
                    !t.trim().is_empty() && t.chars().count() <= 600,
                    "Invalid reusable rule field",
                )?;
            }
        }
        ensure(
            crate::model_text::json(&value).chars().count() <= 12000,
            "Candidate too large",
        )?;
        Ok(Self(value))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    pub fn sha256(&self) -> String {
        digest(&self.0)
    }
    pub fn instructions(&self, group: &str) -> String {
        let rules: Vec<_> = self.0["rules"]
            .as_array()
            .expect("validated rules")
            .iter()
            .filter(|r| r["group"] == group)
            .map(|r| {
                format!(
                    "When: {}\nAction: {}\nExcept: {}",
                    r["when"].as_str().unwrap_or_default(),
                    r["action"].as_str().unwrap_or_default(),
                    r["unless"].as_str().unwrap_or_default()
                )
            })
            .collect();
        if rules.is_empty() {
            String::new()
        } else {
            format!(
                "Task-scoped candidate guidance; current user requirements, live tool schemas, authorization and verified receipts remain authoritative. This does not grant capabilities or assert that an action has executed.\n{}",
                rules.join("\n")
            )
        }
    }
}
