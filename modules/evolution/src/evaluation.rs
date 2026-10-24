// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::contracts::Policy;
use async_trait::async_trait;
use exoharness_benchmark::{
    BenchmarkHarness, ManagementPort, Result, Suite, campaign, digest, ensure, evidence, list,
    metrics::{self, Retention},
    text,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, path::Path};
pub fn reuse_failures(
    policy: &Policy,
    outcome: &Value,
    revision: &Value,
    resource_index: Option<&Value>,
) -> Result<BTreeSet<String>> {
    let mut reasons = BTreeSet::new();
    if !list(&outcome["skill_reads"])?
        .iter()
        .any(|r| r["id"] == policy.skill_id() && r["revision"] == *revision)
    {
        reasons.insert("experience_not_read_in_every_candidate_attempt".into());
    }
    if let Some(index) = resource_index {
        let selected: Vec<_> = list(&outcome["skill_resource_reads"])?
            .iter()
            .filter(|r| r["id"] == policy.skill_id() && r["revision"] == *revision)
            .flat_map(|r| r["resources"].as_array().into_iter().flatten())
            .collect();
        if selected.is_empty()
            || selected.iter().any(|r| {
                !index["resources"].as_array().is_some_and(|expected| {
                    expected
                        .iter()
                        .any(|e| r["path"] == e["path"] && r["sha256"] == e["sha256"])
                })
            })
        {
            reasons.insert("matching_experience_resources_not_read".into());
        }
    }
    Ok(reasons)
}
pub fn normalize(mut value: Value, skill: &str) -> Result<Value> {
    for k in ["capabilities", "capability_evidence"] {
        let values = list(&value[k])?
            .iter()
            .filter(|r| r["id"] != skill)
            .cloned()
            .collect::<Vec<_>>();
        value[k] = json!(values);
    }
    Ok(value)
}
pub async fn stable(port: &dyn BenchmarkHarness, skill: &str) -> Result<Value> {
    let mut config = port.configuration().await?;
    let id = text(&config["profile"]["id"])?;
    config["profile"] = port
        .management()
        .api(
            &format!("/v1/agent-profiles/{}", exoharness_benchmark::encoded(id)),
            None,
            "GET",
            false,
        )
        .await?;
    normalize(config, skill)
}
pub async fn check_environment(
    policy: &Policy,
    baseline: &dyn BenchmarkHarness,
    canary: &dyn BenchmarkHarness,
    production: &dyn ManagementPort,
) -> Result<String> {
    if policy.value()["scope"]["kind"] == "user" {
        ensure(
            [baseline, canary]
                .iter()
                .all(|p| policy.value()["scope"]["user_id"] == p.user_id()),
            "User-scoped evaluation principal mismatch",
        )?;
    }
    let pinned = policy.s("baseline_binary_sha256");
    ensure(
        production.runtime()? == pinned
            && baseline.management().runtime()? == pinned
            && canary.management().runtime()? == pinned,
        "Use the pinned actual production binary",
    )?;
    ensure(
        baseline.portal() != canary.portal()
            && baseline.portal() != production.origin()
            && canary.portal() != production.origin()
            && baseline.management().origin() != canary.management().origin(),
        "Use isolated baseline, canary and production services",
    )?;
    let skill = policy.skill_id();
    let a = stable(baseline, &skill).await?;
    let b = stable(canary, &skill).await?;
    let actual = normalize(
        production.configuration(text(&a["profile"]["id"])?).await?,
        &skill,
    )?;
    ensure(
        digest(&a) == digest(&b) && digest(&a) == digest(&actual),
        "Model, assistant, permissions or unrelated capabilities differ",
    )?;
    let access = &b["profile"]["access"]["skills"];
    ensure(
        access["mode"] == "all"
            || access["ids"]
                .as_array()
                .is_some_and(|ids| ids.contains(&json!(skill))),
        "Canary assistant lacks experience Skill authorization",
    )?;
    baseline.fingerprint().await?;
    canary.fingerprint().await?;
    Ok(digest(&actual))
}
struct Scoped<'a> {
    port: &'a dyn BenchmarkHarness,
    skill: String,
}
#[async_trait]
impl BenchmarkHarness for Scoped<'_> {
    fn run_ceiling(&self) -> u64 {
        self.port.run_ceiling()
    }
    fn user_id(&self) -> &str {
        self.port.user_id()
    }
    fn portal(&self) -> &str {
        self.port.portal()
    }
    fn management(&self) -> &dyn ManagementPort {
        self.port.management()
    }
    async fn configuration(&self) -> Result<Value> {
        self.port.configuration().await
    }
    async fn configuration_fingerprint(&self) -> Result<String> {
        Ok(digest(&stable(self.port, &self.skill).await?))
    }
    async fn fingerprint(&self) -> Result<String> {
        self.port.fingerprint().await
    }
    async fn execute(
        &self,
        root: &Path,
        prompts: &[String],
        fingerprint: &str,
        attachments: &[Value],
    ) -> Result<Value> {
        self.port
            .execute(root, prompts, fingerprint, attachments)
            .await
    }
}
pub fn phase_suite(suite: &Suite, group: &str, split: &str) -> Result<Suite> {
    let mut tasks: Vec<_> = suite
        .tasks()
        .iter()
        .filter(|t| t["group"] == group && (t["split"] == "train" || t["split"] == split))
        .cloned()
        .collect();
    ensure(
        tasks.iter().any(|t| t["split"] == split),
        "Evolution needs separate validation and test tasks",
    )?;
    for task in &mut tasks {
        if task["split"] == split {
            task["split"] = json!("test");
        }
    }
    let id = suite.value()["id"].as_str().expect("suite id");
    let mut value = json!({"version":1,"id":format!("{}_{}",id.chars().take(55).collect::<String>(),split),"tasks":tasks});
    if let Some(rules) = suite.value().get("rules") {
        value["rules"] = rules.clone();
    }
    Suite::parse(value)
}
pub struct Phase<'a> {
    pub split: &'a str,
    pub learning_tokens: u64,
    pub max_tokens: u64,
    pub revision: &'a Value,
    pub resource_index: Option<&'a Value>,
}
pub async fn compare(
    policy: &Policy,
    suite: &Suite,
    baseline: &dyn BenchmarkHarness,
    canary: &dyn BenchmarkHarness,
    root: &Path,
    input_root: Option<&Path>,
    phase: Phase<'_>,
) -> Result<Value> {
    let current = phase_suite(suite, policy.s("group"), phase.split)?;
    let a = Scoped {
        port: baseline,
        skill: policy.skill_id(),
    };
    let b = Scoped {
        port: canary,
        skill: policy.skill_id(),
    };
    let options = campaign::Options {
        repeats: policy.n("repeats"),
        max_tokens: phase.max_tokens,
        run_reservation: policy.n("run_reservation"),
        input_root: input_root.map(ToOwned::to_owned),
    };
    let original = campaign::run(&current, &a, Some(&b), root, &options, false).await?;
    let mut report = original.clone();
    report["learning_tokens"] = json!(phase.learning_tokens);
    let rows = list(&report["rows"])?;
    let ids = list(&report["expected_tasks"])?
        .iter()
        .map(|v| Ok(text(v)?.to_string()))
        .collect::<Result<Vec<_>>>()?;
    let mut decision = metrics::retain(
        rows,
        Retention {
            tasks: &ids,
            repeats: policy.n("repeats"),
            learning_tokens: Some(phase.learning_tokens),
            min_savings: policy.value()["min_savings"].as_f64().expect("fraction"),
            runtimes: Some(&report["runtime_fingerprints"]),
        },
    );
    let mut reasons: std::collections::BTreeSet<String> = list(&decision["reasons"])?
        .iter()
        .map(|v| text(v).map(ToOwned::to_owned))
        .collect::<Result<_>>()?;
    let candidates: Vec<_> = rows.iter().filter(|r| r["mode"] == "candidate").collect();
    if candidates.is_empty() || candidates.iter().any(|r| r["delivered"] != true) {
        reasons.insert("candidate_delivery_failed".into());
    }
    if decision["candidate_tokens_per_delivery_including_learning"]
        .as_f64()
        .is_none_or(|v| v > policy.n("max_tokens_per_delivery") as f64)
    {
        reasons.insert("delivery_token_ceiling".into());
    }
    for row in candidates {
        if row["tokens"]
            .as_u64()
            .is_none_or(|v| v > policy.n("max_tokens_per_delivery"))
        {
            reasons.insert("attempt_token_ceiling".into());
        }
        let outcome = evidence::read(&root.join(text(&row["key"])?).join("outcome.json"))?;
        reasons.extend(reuse_failures(
            policy,
            &outcome,
            phase.revision,
            phase.resource_index,
        )?);
    }
    if list(&original["decision"]["reasons"])?
        .iter()
        .any(|r| r == "model_binding_drift" || r == "runtime_drift")
    {
        reasons.insert("model_or_runtime_drift".into());
    }
    decision["retain"] = json!(reasons.is_empty());
    decision["reasons"] = json!(reasons);
    report["decision"] = decision;
    report["original_suite_sha256"] = json!(suite.sha256());
    report["evaluation_split"] = json!(phase.split);
    report["policy_sha256"] = json!(policy.sha256());
    report["actual_persisted_skill"] = json!(true);
    report["skill_revision"] = phase.revision.clone();
    let reviewfile = root.join("delivery-review.json");
    if reviewfile.exists() {
        let hard_reasons: std::collections::BTreeSet<String> =
            list(&report["decision"]["reasons"])?
                .iter()
                .filter(|r| {
                    *r != "delivery_review_pending" && *r != "no_measured_gain_including_learning"
                })
                .map(|r| text(r).map(ToOwned::to_owned))
                .collect::<Result<_>>()?;
        report = campaign::review(&report, &evidence::read_private(&reviewfile)?)?;
        let mut reviewed = metrics::retain(
            list(&report["rows"])?,
            Retention {
                tasks: &ids,
                repeats: policy.n("repeats"),
                learning_tokens: Some(phase.learning_tokens),
                min_savings: policy.value()["min_savings"].as_f64().expect("fraction"),
                runtimes: Some(&report["runtime_fingerprints"]),
            },
        );
        let mut reasons: std::collections::BTreeSet<String> = list(&reviewed["reasons"])?
            .iter()
            .map(|r| text(r).map(ToOwned::to_owned))
            .collect::<Result<_>>()?;
        reasons.extend(hard_reasons);
        if list(&report["rows"])?
            .iter()
            .any(|r| r["mode"] == "candidate" && r["delivered"] != true)
        {
            reasons.insert("candidate_delivery_failed".into());
        }
        if reviewed["candidate_tokens_per_delivery_including_learning"]
            .as_f64()
            .is_none_or(|v| v > policy.n("max_tokens_per_delivery") as f64)
        {
            reasons.insert("delivery_token_ceiling".into());
        }
        reviewed["retain"] = json!(reasons.is_empty());
        reviewed["reasons"] = json!(reasons);
        report["decision"] = reviewed;
    }
    evidence::write(&root.join("evolution-report.json"), &report)?;
    Ok(report)
}
