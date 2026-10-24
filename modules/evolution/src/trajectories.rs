// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use exoharness_benchmark::{
    Result, Suite, contracts::fields, digest, ensure, evidence, flagged, list, metrics, text,
    verifiers,
};
use serde_json::{Value, json};
use std::path::Path;
#[derive(Clone, Default)]
pub struct Trajectories {
    pub rows: Vec<Value>,
    pub identity: Option<String>,
}
fn read(root: &Path, path: &str) -> Result<Value> {
    ensure(
        !Path::new(path).is_absolute()
            && path.len() <= 240
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_./-".contains(&b)),
        "Invalid evidence path",
    )?;
    let path = root.join(path);
    ensure(
        std::fs::metadata(&path)?.len() <= 8 * 1024 * 1024,
        "Evidence too large",
    )?;
    evidence::read_private(&path)
}
impl Trajectories {
    pub fn load(
        directory: Option<&Path>,
        suite: &Suite,
        group: &str,
        owner: Option<&str>,
    ) -> Result<Self> {
        let Some(root) = directory else {
            return Ok(Self::default());
        };
        evidence::private_dir(root)?;
        let manifest = read(root, "manifest.json")?;
        fields(&manifest, &["version", "suite_sha256", "samples"], true)?;
        ensure(
            manifest["version"] == 1 && manifest["suite_sha256"] == suite.sha256(),
            "Training evidence suite mismatch",
        )?;
        let samples = list(&manifest["samples"])?;
        ensure(
            (1..=16).contains(&samples.len()),
            "Need 1..16 reviewed samples",
        )?;
        let mut result = Self::default();
        let mut seen = std::collections::BTreeSet::new();
        for sample in samples {
            fields(
                sample,
                &[
                    "task_id",
                    "directory",
                    "outcome_sha256",
                    "session_sha256",
                    "trace_sha256",
                    "owner_user_id",
                    "reviewer",
                    "delivery_passed",
                ],
                true,
            )?;
            let task = suite
                .tasks()
                .iter()
                .find(|t| {
                    t["id"] == sample["task_id"] && t["split"] == "train" && t["group"] == group
                })
                .ok_or_else(|| {
                    exoharness_benchmark::Error::Invalid(
                        "Only matching training tasks may supply trajectories".into(),
                    )
                })?;
            ensure(
                !text(&sample["reviewer"])?.trim().is_empty()
                    && text(&sample["reviewer"])?.chars().count() <= 120
                    && sample["delivery_passed"].is_boolean()
                    && !text(&sample["owner_user_id"])?.is_empty()
                    && owner.is_none_or(|v| sample["owner_user_id"] == v),
                "Invalid reviewed owner or delivery",
            )?;
            let dir = text(&sample["directory"])?;
            let outcome = read(root, &format!("{dir}/outcome.json"))?;
            let execution = read(root, &format!("{dir}/execution.json"))?;
            let session = read(root, &format!("{dir}/session.json"))?;
            ensure(
                digest(&session) == sample["session_sha256"]
                    && session["id"] == execution["session_id"]
                    && session["owner_user_id"] == sample["owner_user_id"]
                    && digest(&outcome) == sample["outcome_sha256"],
                "Reviewed ownership or outcome changed",
            )?;
            let turns = list(&execution["turns"])?;
            let runs = list(&outcome["run_ids"])?;
            let hashes = list(&sample["trace_sha256"])?;
            let prompts = list(&task["turns"])?;
            ensure(
                !prompts.is_empty()
                    && [turns.len(), runs.len(), hashes.len()]
                        .iter()
                        .all(|n| *n == prompts.len()),
                "Evidence must cover every training Turn",
            )?;
            let (mut calls, mut steps, mut states) = (Vec::new(), Vec::new(), Vec::new());
            for (index, prompt) in prompts.iter().enumerate() {
                let rid = &runs[index];
                ensure(
                    seen.insert(rid.to_string())
                        && turns[index]["run_id"] == *rid
                        && turns[index]["prompt_sha256"] == digest(prompt),
                    "Repeated Run or input mismatch",
                )?;
                let trace = read(root, &format!("{dir}/trace-{index}.json"))?;
                let run = &trace["run"];
                ensure(
                    digest(&trace) == hashes[index]
                        && run["id"] == *rid
                        && trace["prompt"] == *prompt
                        && run["session_id"] == session["id"]
                        && ["succeeded", "failed", "recoverable"].contains(&text(&run["status"])?),
                    "Trace identity, ownership or terminal state mismatch",
                )?;
                states.push(run["status"].clone());
                calls.extend(list(&trace["model_calls"])?.iter().cloned());
                for invocation in list(&trace["invocations"])? {
                    let name = text(&invocation["tool_name"])?;
                    ensure(
                        !name.is_empty()
                            && name.len() <= 80
                            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
                        "Invalid recorded tool name",
                    )?;
                    let r = &invocation["result"];
                    let success = invocation["outcome_status"] == "success"
                        && !flagged(&r["isError"])
                        && !flagged(&r["is_error"])
                        && !flagged(&r["_host_input_rejection"])
                        && !flagged(&r["_host_authorization_denied"]);
                    let failure = if flagged(&r["_host_authorization_denied"]) {
                        "capability_denied"
                    } else if flagged(&r["_host_input_rejection"]) {
                        "input_invalid"
                    } else if invocation["outcome_status"].is_null()
                        || ["unknown", "submitted", "running"]
                            .contains(&invocation["outcome_status"].as_str().unwrap_or(""))
                    {
                        "unknown_submission"
                    } else {
                        "execution_failed"
                    };
                    steps.push(json!({"tool":name,"outcome":if success{"success"}else{failure}}));
                }
            }
            ensure(
                exoharness_benchmark::model_text::json(&json!(steps))
                    .chars()
                    .count()
                    <= 16000,
                "Trajectory exceeds induction window",
            )?;
            let usage = metrics::usage(&calls);
            ensure(
                !calls.is_empty()
                    && usage["unknown_model_calls"] == 0
                    && outcome["tokens"] == usage["total_tokens"]
                    && outcome["runtime_drift"] != true
                    && outcome["turn_states"] == json!(states)
                    && outcome["state"] == *states.last().expect("nonempty turns"),
                "Unknown usage or inconsistent runtime evidence",
            )?;
            let delivered = verifiers::verify(task, &outcome)["delivered"] == true
                && states.iter().all(|v| v == "succeeded");
            ensure(
                sample["delivery_passed"] != true || delivered,
                "Review cannot convert failed execution to a workflow",
            )?;
            let verified = delivered && sample["delivery_passed"] == true;
            let mut failures: std::collections::BTreeSet<_> = steps
                .iter()
                .filter(|s| s["outcome"] != "success")
                .map(|s| s["outcome"].clone().to_string())
                .collect();
            if !verified {
                failures.insert(json!("delivery_invalid").to_string());
            }
            let failures: Vec<Value> = failures
                .into_iter()
                .map(|s| serde_json::from_str(&s))
                .collect::<std::result::Result<_, _>>()?;
            result.rows.push(json!({"train_id":task["id"],"verified_delivery":verified,"state":states.last(),"tokens":outcome["tokens"],"steps":steps,"failure_classes":failures,"evidence_sha256":digest(&json!({"outcome":sample["outcome_sha256"],"traces":hashes}))}));
        }
        result.identity = Some(digest(
            &json!({"manifest":manifest,"projection":result.rows}),
        ));
        Ok(result)
    }
}
