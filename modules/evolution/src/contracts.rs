// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::trajectories::Trajectories;
use exoharness_benchmark::{
    Experience, Result, Suite,
    contracts::{fields, hash},
    digest, ensure, identifier, list, number, text,
};
use serde_json::{Value, json};
#[derive(Clone, Debug)]
pub struct Policy(Value);
#[derive(Clone, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub version: u64,
    pub id: String,
    pub policy: std::path::PathBuf,
    pub suite: std::path::PathBuf,
    pub baseline_auth: std::path::PathBuf,
    pub canary_auth: std::path::PathBuf,
    pub publication_auth: std::path::PathBuf,
    pub input_root: std::path::PathBuf,
    pub training_evidence: Option<std::path::PathBuf>,
}
impl Job {
    pub fn parse(value: Value) -> Result<Self> {
        let job: Self = serde_json::from_value(value)?;
        ensure(
            job.version == 1 && identifier(&job.id),
            "Invalid evolution job identity",
        )?;
        Ok(job)
    }
}
impl Policy {
    pub fn parse(mut value: Value) -> Result<Self> {
        fields(
            &value,
            &[
                "version",
                "id",
                "group",
                "skill_name",
                "description",
                "scope",
                "baseline_binary_sha256",
                "suite_sha256",
                "max_rounds",
                "max_tokens",
                "run_reservation",
                "repeats",
                "interval_seconds",
                "min_savings",
                "max_tokens_per_delivery",
                "max_rule_characters",
                "publish",
                "monitor_user_ids",
                "monitor_min_samples",
                "monitor_failure_rate",
                "monitor_max_tokens",
                "auto_suspend",
                "supervision",
                "max_training_characters",
                "method",
            ],
            false,
        )?;
        ensure(value["version"] == 1, "Invalid evolution policy version")?;
        for k in ["id", "group", "skill_name"] {
            ensure(identifier(text(&value[k])?), "Invalid policy identity")?;
        }
        ensure(
            text(&value["skill_name"])?.starts_with("experience-"),
            "Evolution manages only experience-* Skills",
        )?;
        let description = text(&value["description"])?;
        ensure(
            !description.trim().is_empty()
                && description.chars().count() <= 320
                && !description.contains(['\r', '\n', '\0']),
            "Invalid single-line Skill description",
        )?;
        for k in ["baseline_binary_sha256", "suite_sha256"] {
            ensure(
                hash(&value[k]),
                "Pin the actual executable and complete suite",
            )?;
        }
        let defaults = json!({"scope":{"kind":"global"},"max_rounds":3,"max_tokens":1000000,"run_reservation":300000,"repeats":3,"interval_seconds":3600,"min_savings":0.05,"max_tokens_per_delivery":100000,"max_rule_characters":2400,"publish":false,"monitor_user_ids":[],"monitor_min_samples":5,"monitor_failure_rate":0.2,"monitor_max_tokens":100000,"auto_suspend":false,"supervision":"reflect","max_training_characters":32000});
        for (k, v) in defaults.as_object().expect("defaults") {
            if value.get(k).is_none() {
                value[k] = v.clone();
            }
        }
        let scope = &value["scope"];
        ensure(
            *scope == json!({"kind":"global"})
                || (scope["kind"] == "user"
                    && scope.as_object().is_some_and(|m| m.len() == 2)
                    && text(&scope["user_id"]).is_ok_and(|s| {
                        !s.is_empty()
                            && s.len() <= 128
                            && s.bytes()
                                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
                    })),
            "Invalid publication scope",
        )?;
        for (k, low, high) in [
            ("max_rounds", 1, 20),
            ("max_tokens", 1, 100000000),
            ("run_reservation", 1, 1000000),
            ("repeats", 3, 5),
            ("interval_seconds", 60, 604800),
            ("max_tokens_per_delivery", 1, 1000000),
            ("max_rule_characters", 200, 4800),
            ("monitor_min_samples", 2, 1000),
            ("monitor_max_tokens", 1, 1000000),
            ("max_training_characters", 1000, 64000),
        ] {
            ensure(
                (low..=high).contains(&number(&value[k])?),
                "Invalid bounded policy limit",
            )?;
        }
        ensure(
            number(&value["max_tokens"])? >= number(&value["run_reservation"])?,
            "Budget cannot admit one reserved Run",
        )?;
        for k in ["min_savings", "monitor_failure_rate"] {
            ensure(
                value[k].as_f64().is_some_and(|v| (0.0..=1.0).contains(&v)),
                "Invalid policy fraction",
            )?;
        }
        ensure(
            value["publish"].is_boolean() && value["auto_suspend"].is_boolean(),
            "Publication/suspension need explicit booleans",
        )?;
        let owners = list(&value["monitor_user_ids"])?;
        let mut known = std::collections::BTreeSet::new();
        ensure(owners.len() <= 100, "Too many monitoring owners")?;
        for owner in owners {
            let s = text(owner)?;
            ensure(
                !s.is_empty() && known.insert(s.to_string()),
                "Invalid monitoring owner",
            )?;
        }
        ensure(
            value["auto_suspend"] != true || !owners.is_empty(),
            "Suspension requires scoped observation",
        )?;
        ensure(
            ["self", "fewshot", "reflect"].contains(&text(&value["supervision"])?),
            "Unknown supervision mode",
        )?;
        let method = value
            .get("method")
            .map(text)
            .transpose()?
            .unwrap_or("rules");
        ensure(
            ["rules", "workflow"].contains(&method)
                && !(method == "workflow" && value["supervision"] == "self"),
            "Invalid learning method/supervision",
        )?;
        Ok(Self(value))
    }
    pub fn value(&self) -> &Value {
        &self.0
    }
    pub fn sha256(&self) -> String {
        digest(&self.0)
    }
    pub fn n(&self, k: &str) -> u64 {
        self.0[k].as_u64().expect("validated numeric policy")
    }
    pub fn s(&self, k: &str) -> &str {
        self.0[k].as_str().expect("validated text policy")
    }
    pub fn method(&self) -> &str {
        self.0["method"].as_str().unwrap_or("rules")
    }
    pub fn skill_id(&self) -> String {
        if self.0["scope"]["kind"] == "global" {
            format!("skill:global:_:{}", self.s("skill_name"))
        } else {
            format!(
                "skill:user:{}:{}",
                self.0["scope"]["user_id"].as_str().expect("scope owner"),
                self.s("skill_name")
            )
        }
    }
}
pub fn safe_text(value: &Value, maximum: usize) -> Result<()> {
    let s = text(value)?;
    ensure(
        !s.trim().is_empty()
            && s.chars().count() <= maximum
            && !s.chars().any(|c| c < ' ' && !['\n', '\t'].contains(&c)),
        "Invalid bounded reusable text",
    )?;
    let sensitive=regex::Regex::new(r"(?i)https?://|[\w.+-]+@[\w.-]+\.[A-Za-z]{2,}|Bearer\s|(?:session-|run-|artifact-|vid_)\w{8,}|(?:api[_-]?key|password|access_token)\s*[:=]").expect("constant pattern");
    ensure(
        !sensitive.is_match(s),
        "Candidate contains a private handle, URL or credential",
    )
}
fn terms(value: &Value) -> Result<()> {
    let values = list(value)?;
    ensure(
        !values.is_empty() && values.len() <= 8,
        "Need 1..8 search terms",
    )?;
    let mut seen = std::collections::BTreeSet::new();
    for value in values {
        let s = text(value)?;
        ensure(
            (2..=40).contains(&s.chars().count())
                && s == s.trim().to_lowercase()
                && seen.insert(s.to_string()),
            "Invalid normalized search terms",
        )?;
        safe_text(value, 40)?;
    }
    Ok(())
}
pub fn candidate(
    value: Value,
    policy: &Policy,
    suite: &Suite,
    trajectories: &Trajectories,
) -> Result<Value> {
    if policy.method() != "workflow" {
        let parsed = Experience::parse(value, suite)?;
        ensure(
            list(&parsed.value()["rules"])?
                .iter()
                .all(|r| r["group"] == policy.value()["group"]),
            "Learner changed group scope",
        )?;
        return Ok(parsed.value().clone());
    }
    fields(
        &value,
        &["version", "suite_sha256", "rules", "workflows"],
        true,
    )?;
    ensure(
        value["version"] == 2 && value["suite_sha256"] == suite.sha256(),
        "Workflow candidate identity mismatch",
    )?;
    let rules = list(&value["rules"])?;
    let workflows = list(&value["workflows"])?;
    ensure(
        (1..=8).contains(&(rules.len() + workflows.len())) && workflows.len() <= 4,
        "Candidate exceeds experience count",
    )?;
    if !rules.is_empty() {
        let reduced: Vec<_> = rules
            .iter()
            .map(|r| {
                let mut r = r.clone();
                if let Some(o) = r.as_object_mut() {
                    o.remove("terms");
                }
                r
            })
            .collect();
        Experience::parse(
            json!({"version":1,"suite_sha256":suite.sha256(),"rules":reduced}),
            suite,
        )?;
    }
    for rule in rules {
        fields(
            rule,
            &["group", "sources", "when", "action", "unless", "terms"],
            true,
        )?;
        ensure(
            rule["group"] == policy.value()["group"]
                && list(&rule["sources"])?
                    .iter()
                    .all(|s| trajectories.rows.iter().any(|r| r["train_id"] == *s)),
            "Lessons require reviewed matching training sources",
        )?;
        terms(&rule["terms"])?;
        for k in ["when", "action", "unless"] {
            safe_text(&rule[k], 600)?;
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    for workflow in workflows {
        fields(
            workflow,
            &["id", "group", "sources", "when", "unless", "terms", "steps"],
            true,
        )?;
        ensure(
            identifier(text(&workflow["id"])?)
                && ids.insert(workflow["id"].to_string())
                && workflow["group"] == policy.value()["group"],
            "Invalid workflow identity/group",
        )?;
        let sources = list(&workflow["sources"])?;
        let mut seen = std::collections::BTreeSet::new();
        ensure(!sources.is_empty(), "Workflow needs sources")?;
        for s in sources {
            ensure(
                seen.insert(s.to_string())
                    && trajectories
                        .rows
                        .iter()
                        .any(|r| r["train_id"] == *s && r["verified_delivery"] == true),
                "Workflow source must be a successful reviewed delivery",
            )?;
        }
        for k in ["when", "unless"] {
            safe_text(&workflow[k], 300)?;
        }
        terms(&workflow["terms"])?;
        let steps = list(&workflow["steps"])?;
        ensure(!steps.is_empty(), "Workflow needs observed steps")?;
        for step in steps {
            fields(step, &["tool", "action", "expect"], true)?;
            safe_text(&step["action"], 300)?;
            safe_text(&step["expect"], 240)?;
        }
        ensure(
            trajectories
                .rows
                .iter()
                .filter(|r| r["verified_delivery"] == true && sources.contains(&r["train_id"]))
                .any(|r| {
                    let mut observed = r["steps"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter(|s| s["outcome"] == "success");
                    steps
                        .iter()
                        .all(|step| observed.any(|s| s["tool"] == step["tool"]))
                }),
            "Workflow tool order is not an observed successful subsequence",
        )?;
    }
    ensure(
        exoharness_benchmark::model_text::json(&json!({"rules":rules,"workflows":workflows}))
            .chars()
            .count()
            <= policy.n("max_rule_characters") as usize,
        "Complete workflow exceeds context budget",
    )?;
    Ok(value)
}
