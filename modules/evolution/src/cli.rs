// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{
    contracts::{Job, Policy},
    controller::Environment,
    package,
    trajectories::Trajectories,
};
use exoharness_benchmark::{Result, Suite, cli::Args, digest, ensure, evidence};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
pub async fn main(raw: &[String]) -> Result<Value> {
    let action = raw
        .first()
        .ok_or_else(|| exoharness_benchmark::Error::Invalid("Evolution action required".into()))?;
    if action == "skill-package" {
        let a = Args::parse(&raw[1..], &["--output"])?;
        evidence::write(a.path("--output")?, &package::learning_skill())?;
        return Ok(json!({"output":a.required("--output")?,"skill":"workflow-evolution"}));
    }
    if action == "validate" {
        let a = Args::parse(&raw[1..], &["--kind", "--input", "--training-evidence"])?;
        let v = evidence::read(a.path("--input")?)?;
        if a.required("--kind")? == "policy" {
            let p = Policy::parse(v)?;
            return Ok(json!({"value":p.value(),"sha256":p.sha256(),"skill_id":p.skill_id()}));
        }
        let suite = Suite::parse(v["suite"].clone())?;
        let policy = Policy::parse(v["policy"].clone())?;
        let trajectory = Trajectories::load(
            a.optional("--training-evidence").map(Path::new),
            &suite,
            policy.s("group"),
            policy.value()["scope"]["user_id"].as_str(),
        )?;
        return match a.required("--kind")? {
            "prompt" => {
                let feedback = v
                    .get("feedback")
                    .map(exoharness_benchmark::list)
                    .transpose()?
                    .cloned()
                    .unwrap_or_default();
                Ok(
                    json!({"prompt":crate::learning::prompt(&policy,&suite,&v["incumbent"],&feedback,&trajectory)?}),
                )
            }
            "candidate" => {
                let value = crate::contracts::candidate(
                    v["candidate"].clone(),
                    &policy,
                    &suite,
                    &trajectory,
                )?;
                Ok(json!({"sha256":digest(&value),"value":value}))
            }
            "package" => {
                let body = package::compile(
                    &policy,
                    &suite,
                    &v["candidate"],
                    "round-0",
                    v["runs"].as_array().map(Vec::as_slice).unwrap_or(&[]),
                    &trajectory,
                )?;
                Ok(json!({"source_digest":package::package_digest(&body)?,"body":body}))
            }
            "trajectories" => Ok(json!({"identity":trajectory.identity,"rows":trajectory.rows})),
            _ => Err(exoharness_benchmark::Error::Invalid(
                "Unknown evolution validation kind".into(),
            )),
        };
    }
    if action == "watch" {
        let a = Args::parse(
            &raw[1..],
            &[
                "--queue",
                "--state-root",
                "--max-tokens",
                "--interval-seconds",
                "--cycles",
                "--allow-model-requests",
            ],
        )?;
        ensure(
            a.flag("--allow-model-requests"),
            "Watch requires --allow-model-requests",
        )?;
        let interval = a.n("--interval-seconds", 3600)?;
        ensure(interval >= 60, "Watch interval must be at least 60 seconds")?;
        let cycles = a.n("--cycles", 1)?;
        let mut cycle = 0;
        loop {
            let result = crate::queue::tick(
                a.path("--queue")?,
                a.path("--state-root")?,
                a.n("--max-tokens", 0)?,
            )
            .await?;
            cycle += 1;
            if cycles != 0 && cycle >= cycles {
                return Ok(result);
            }
            println!("{result}");
            tokio::select! {_=tokio::time::sleep(Duration::from_secs(interval))=>{},_=tokio::signal::ctrl_c()=>return Ok(result)}
        }
    }
    ensure(
        ["run", "publish", "monitor", "status"].contains(&action.as_str()),
        "Unknown evolution action",
    )?;
    let a = Args::parse(
        &raw[1..],
        &["--job", "--state-root", "--allow-model-requests"],
    )?;
    if action == "run" {
        ensure(
            a.flag("--allow-model-requests"),
            "Learning requires --allow-model-requests",
        )?;
    }
    let job = Job::parse(evidence::read_private(a.path("--job")?)?)?;
    let root = a.path("--state-root")?.join(&job.id);
    if action == "status" {
        let state = evidence::read(&root.join("evolution.json"))?;
        return Ok(
            json!({"phase":state["phase"],"charged_tokens":exoharness_benchmark::campaign::charges(&state)?,"rounds":state["rounds"],"publications":state["publications"]}),
        );
    }
    let environment = Environment::load(&job)?;
    let controller = environment.controller(&job, &root)?;
    match action.as_str() {
        "run" => controller.run().await,
        "publish" => controller.publish().await,
        "monitor" => controller.monitor().await,
        _ => unreachable!("validated action"),
    }
}
