// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use super::{Dataset, Gateway, Grader};
use crate::{
    BenchmarkHarness, Result,
    campaign::{self, Options},
    ensure, evidence, list, metrics, text, verifiers,
};
use serde_json::{Value, json};
use std::path::Path;
#[allow(clippy::too_many_arguments)]
pub async fn run(
    dataset: &Dataset,
    group: &str,
    port: &dyn BenchmarkHarness,
    grader: &Grader,
    root: &Path,
    environment: &str,
    repeats: u64,
    max_tokens: u64,
    owned_environment: bool,
) -> Result<Value> {
    ensure(
        owned_environment && (3..=5).contains(&repeats) && max_tokens >= port.run_ceiling(),
        "Require owned disposable environment, 3..5 repetitions and sufficient budget",
    )?;
    let _lock = evidence::Exclusive::acquire(root)?;
    let pinpath = root.join("gateway.json");
    let pin = if pinpath.exists() {
        evidence::read(&pinpath)?
    } else {
        json!({})
    };
    let pinned = pin["port"].as_u64().unwrap_or(0);
    ensure(pinned <= 65535, "Invalid saved gateway port")?;
    let gateway = Gateway::start(dataset, group, environment, pinned as u16).await?;
    let gatewayport = url::Url::parse(&gateway.origin)
        .map_err(|_| crate::Error::Invalid("Invalid gateway origin".into()))?
        .port()
        .expect("gateway port");
    evidence::write(&pinpath, &json!({"port":gatewayport}))?;
    let mutable = dataset.environment(group)?["state_mode"] == "mutable";
    let fingerprint = port.fingerprint().await?;
    let identity = json!({"revision":dataset.revision,"inventory_sha256":dataset.inventory()?["dataset_sha256"],"group":group,"runtime":fingerprint,"environment_url":environment,"gateway":gateway.origin,"repeats":repeats,"max_tokens":max_tokens,"grading_image":grader.image});
    let statepath = root.join("gdpevo.json");
    let mut state = campaign::load_state(&statepath, identity.clone())?;
    let options = Options {
        repeats,
        max_tokens,
        run_reservation: port.run_ceiling(),
        input_root: None,
    };
    for attempt in 0..repeats {
        for task in dataset.task_ids(group, "test")? {
            let key = format!("{task}-{attempt}");
            if list(&state["rows"])?.iter().any(|r| r["key"] == key) {
                continue;
            }
            ensure(
                port.fingerprint().await? == fingerprint,
                "Runtime drift; no submission",
            )?;
            if mutable && !root.join(&key).join("execution.json").exists() {
                gateway.reset_owned().await?;
            }
            let prompt = dataset
                .solver_input(group, "test", &task, &gateway.origin)?
                .to_string();
            let outcome = campaign::execute_charged(
                &mut state,
                &statepath,
                &key,
                port,
                &[prompt],
                &fingerprint,
                &[],
                &options,
            )
            .await?;
            let prediction = verifiers::json_answer(text(&outcome["text"])?).unwrap_or(json!({}));
            let receipt = grader
                .grade(
                    group,
                    "test",
                    &task,
                    &prediction,
                    &root.join(&key).join("grade.json"),
                )
                .await?;
            state["rows"].as_array_mut().expect("rows").push(json!({"key":key,"tokens":outcome["tokens"],"state":outcome["state"],"delivered":outcome["state"]=="succeeded"&&receipt["passed"]==true,"score":receipt["score"],"run_ids":outcome["run_ids"],"elapsed_ms":outcome["elapsed_ms"]}));
            evidence::write(&statepath, &state)?;
        }
    }
    let mut report = identity;
    let rows = list(&state["rows"])?;
    ensure(!rows.is_empty(), "No official evaluation attempts")?;
    report["rows"] = state["rows"].clone();
    report["summary"] = metrics::aggregate(rows);
    report["charged_tokens"] = json!(campaign::charges(&state)?);
    report["mean_official_score"] =
        json!(rows.iter().filter_map(|r| r["score"].as_f64()).sum::<f64>() / rows.len() as f64);
    evidence::write(&root.join("report.json"), &report)?;
    Ok(report)
}
