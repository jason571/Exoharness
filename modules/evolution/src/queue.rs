// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{contracts::Job, controller::Environment};
use exoharness_benchmark::{Result, digest, ensure, evidence, harness::now_ms};
use serde_json::{Value, json};
use std::path::Path;
pub async fn tick(queue: &Path, root: &Path, limit: u64) -> Result<Value> {
    evidence::private_dir(queue)?;
    let _lock = evidence::Exclusive::acquire(root)?;
    let path = root.join("queue.json");
    let mut state = if path.exists() {
        evidence::read(&path)?
    } else {
        json!({"limit":limit,"jobs":{}})
    };
    ensure(
        limit > 0 && state["limit"] == limit,
        "Queue Token ceiling is immutable",
    )?;
    evidence::write(&path, &state)?;
    let mut paths = std::fs::read_dir(queue)?
        .map(|e| e.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|p| p.extension().is_some_and(|s| s == "json"));
    paths.sort();
    ensure(paths.len() <= 100, "Queue is bounded to 100 campaigns")?;
    for jobfile in paths {
        let value = evidence::read_private(&jobfile)?;
        let job = Job::parse(value.clone())?;
        let key = &job.id;
        let identity = digest(&value);
        if let Some(saved) = state["jobs"].get(key) {
            ensure(
                saved["job_sha256"] == identity,
                "Queued job changed; use a new identity",
            )?;
            if saved["phase"] == "blocked" {
                continue;
            }
        }
        let environment = Environment::load(&job)?;
        let controller = environment.controller(&job, &root.join(key))?;
        if state["jobs"].get(key).is_none() {
            let spent = state["jobs"]
                .as_object()
                .expect("jobs")
                .values()
                .map(|v| v["charged_tokens"].as_u64().unwrap_or(limit))
                .try_fold(0u64, |a, b| a.checked_add(b))
                .unwrap_or(u64::MAX);
            let reserve = controller.policy.n("max_tokens");
            if spent.checked_add(reserve).is_none_or(|v| v > limit) {
                continue;
            }
            state["jobs"][key] =
                json!({"job_sha256":identity,"charged_tokens":reserve,"phase":"admitted"});
            evidence::write(&path, &state)?;
        }
        let result = async {
            if state["jobs"][key]["phase"] == "published" {
                if (now_ms() / 1000)
                    .saturating_sub(state["jobs"][key]["last_monitor_at"].as_u64().unwrap_or(0))
                    >= controller.policy.n("interval_seconds")
                {
                    state["jobs"][key]["monitor"] = controller.monitor().await?;
                    state["jobs"][key]["last_monitor_at"] = json!(now_ms() / 1000);
                }
                return Ok::<(), exoharness_benchmark::Error>(());
            }
            let result = controller.run().await?;
            let terminal = [
                "published",
                "validated",
                "validation_rejected",
                "test_rejected",
                "unchanged",
                "exhausted",
            ]
            .contains(&result["phase"].as_str().unwrap_or(""));
            let ledger = evidence::read(&controller.root.join("evolution.json"))?;
            state["jobs"][key]["charged_tokens"] = json!(if terminal {
                exoharness_benchmark::campaign::charges(&ledger)?
            } else {
                controller.policy.n("max_tokens")
            });
            state["jobs"][key]["phase"] = result["phase"].clone();
            if result["phase"] == "published" {
                state["jobs"][key]["monitor"] = controller.monitor().await?;
                state["jobs"][key]["last_monitor_at"] = json!(now_ms() / 1000);
            }
            Ok(())
        }
        .await;
        if result.is_err() {
            state["jobs"][key]["phase"] = json!("blocked");
            state["jobs"][key]["error_class"] = json!("EvolutionCampaignFailed");
            let ledger = controller.root.join("evolution.json");
            if ledger.exists() {
                state["jobs"][key]["charged_tokens"] = json!(
                    exoharness_benchmark::campaign::charges(&evidence::read(&ledger)?)?
                );
            }
        }
        evidence::write(&path, &state)?;
    }
    Ok(state)
}
