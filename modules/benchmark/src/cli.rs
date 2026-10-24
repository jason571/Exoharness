// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Harness, Result, Suite, campaign, ensure, evidence, gdpevo, scenarios};
use serde_json::{Value, json};
use std::{collections::BTreeMap, path::Path};
pub struct Args {
    values: BTreeMap<String, String>,
}
impl Args {
    pub fn parse(raw: &[String], allowed: &[&str]) -> Result<Self> {
        let mut values = BTreeMap::new();
        let mut i = 0;
        while i < raw.len() {
            let flag = raw[i].as_str();
            ensure(allowed.contains(&flag), "Unknown command option")?;
            let boolean = ["--allow-model-requests", "--owned-environment"].contains(&flag);
            let value = if boolean {
                "true".to_string()
            } else {
                i += 1;
                raw.get(i)
                    .filter(|v| !v.starts_with("--"))
                    .cloned()
                    .ok_or_else(|| crate::Error::Invalid("Missing command option value".into()))?
            };
            ensure(
                values.insert(flag.to_string(), value).is_none(),
                "Duplicate command option",
            )?;
            i += 1;
        }
        Ok(Self { values })
    }
    pub fn optional(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }
    pub fn required(&self, key: &str) -> Result<&str> {
        self.optional(key)
            .ok_or_else(|| crate::Error::Invalid(format!("Required option: {key}")))
    }
    pub fn path(&self, key: &str) -> Result<&Path> {
        Ok(Path::new(self.required(key)?))
    }
    pub fn n(&self, key: &str, default: u64) -> Result<u64> {
        self.optional(key)
            .map(|s| {
                s.parse::<u64>()
                    .map_err(|_| crate::Error::Invalid("Invalid nonnegative command limit".into()))
            })
            .unwrap_or(Ok(default))
    }
    pub fn flag(&self, key: &str) -> bool {
        self.optional(key) == Some("true")
    }
}
pub async fn main(raw: &[String]) -> Result<Value> {
    let Some(action) = raw.first() else {
        return Err(crate::Error::Invalid("Usage: exoharness-benchmark suite|plan|run|compare|accept|review|gdpevo|validate [options]".into()));
    };
    if action == "gdpevo" {
        return official(&raw[1..]).await;
    }
    if action == "suite" {
        let a = Args::parse(&raw[1..], &["--kind", "--output", "--mv-audio"])?;
        let suite = scenarios::create(
            a.path("--output")?,
            a.required("--kind")?,
            a.optional("--mv-audio").map(Path::new),
        )?;
        return Ok(json!({"suite_sha256":suite.sha256(),"tasks":suite.tasks().len()}));
    }
    if action == "validate" {
        let a = Args::parse(&raw[1..], &["--kind", "--input"])?;
        let v = evidence::read(a.path("--input")?)?;
        return match a.required("--kind")? {
            "suite" => {
                let suite = Suite::parse(v)?;
                Ok(json!({"sha256":suite.sha256(),"training_material":suite.training_material()}))
            }
            "learning-prompt" => {
                let suite = Suite::parse(v)?;
                Ok(json!({"prompt":campaign::learning_prompt(&suite)}))
            }
            "experience" => {
                let suite = Suite::parse(v["suite"].clone())?;
                let e = crate::Experience::parse(v["candidate"].clone(), &suite)?;
                Ok(
                    json!({"sha256":e.sha256(),"instructions":e.instructions(v["group"].as_str().unwrap_or(""))}),
                )
            }
            "usage" => Ok(crate::metrics::usage(crate::list(&v)?)),
            "aggregate" => Ok(crate::metrics::aggregate(crate::list(&v)?)),
            "verify" => Ok(crate::verifiers::verify(&v["task"], &v["outcome"])),
            _ => Err(crate::Error::Invalid("Unknown validation kind".into())),
        };
    }
    let a = Args::parse(
        &raw[1..],
        &[
            "--suite",
            "--report",
            "--reviews",
            "--allow-model-requests",
            "--auth-file",
            "--candidate-auth-file",
            "--output",
            "--repeats",
            "--max-tokens",
            "--run-reservation",
        ],
    )?;
    if action == "review" {
        let output = a.path("--output")?;
        ensure(!output.exists(), "Review cannot replace original evidence")?;
        let result = campaign::review(
            &evidence::read(a.path("--report")?)?,
            &evidence::read_private(a.path("--reviews")?)?,
        )?;
        evidence::write(output, &result)?;
        return Ok(result);
    }
    if ["run", "compare", "accept"].contains(&action.as_str()) {
        ensure(
            a.flag("--allow-model-requests"),
            "Execution requires --allow-model-requests",
        )?;
    }
    let suitepath = a.path("--suite")?;
    let suite = Suite::parse(evidence::read(suitepath)?)?;
    let repeats = a.n("--repeats", if action == "accept" { 2 } else { 3 })?;
    if action == "plan" {
        return Ok(
            json!({"suite":suite.value()["id"],"sha256":suite.sha256(),"train_tasks":suite.tasks().iter().filter(|t|t["split"]=="train").count(),"held_out_tasks":suite.tasks().iter().filter(|t|t["split"]=="test").count(),"regression_tasks":suite.tasks().iter().filter(|t|t["split"]=="regression").count(),"solver_attempts":suite.tasks().iter().filter(|t|t["split"]!="train").count() as u64*repeats*2,"model_requests_submitted":0}),
        );
    }
    ensure(
        ["run", "compare", "accept"].contains(&action.as_str()),
        "Unknown benchmark action",
    )?;
    ensure(
        a.flag("--allow-model-requests"),
        "Execution requires --allow-model-requests",
    )?;
    let reservation = a.n("--run-reservation", 300000)?;
    let baseline = Harness::new(a.path("--auth-file")?, reservation, 2400)?;
    let candidate = if action == "compare" {
        Some(Harness::new(
            a.path("--candidate-auth-file")?,
            reservation,
            2400,
        )?)
    } else {
        None
    };
    let options = campaign::Options {
        repeats,
        max_tokens: a.n("--max-tokens", 1000000)?,
        run_reservation: reservation,
        input_root: Some(
            suitepath
                .canonicalize()?
                .parent()
                .expect("suite parent")
                .to_owned(),
        ),
    };
    campaign::run(
        &suite,
        &baseline,
        candidate
            .as_ref()
            .map(|p| p as &dyn crate::BenchmarkHarness),
        a.path("--output")?,
        &options,
        action == "accept",
    )
    .await
}
async fn official(raw: &[String]) -> Result<Value> {
    let action = raw
        .first()
        .ok_or_else(|| crate::Error::Invalid("GDPevo action required".into()))?;
    let a = Args::parse(
        &raw[1..],
        &[
            "--checkout",
            "--revision",
            "--group",
            "--split",
            "--task",
            "--environment-origin",
            "--prediction",
            "--grading-image",
            "--output",
            "--auth",
            "--repeats",
            "--max-tokens",
            "--run-reservation",
            "--allow-model-requests",
            "--owned-environment",
        ],
    )?;
    let dataset = gdpevo::Dataset::new(a.path("--checkout")?, a.required("--revision")?)?;
    let value = match action.as_str() {
        "inspect" => dataset.inventory()?,
        "environment" => dataset.environment(a.required("--group")?)?,
        "input" => dataset.solver_input(
            a.required("--group")?,
            a.optional("--split").unwrap_or("test"),
            a.required("--task")?,
            a.required("--environment-origin")?,
        )?,
        "grade" => {
            return gdpevo::Grader::new(dataset, a.required("--grading-image")?)?
                .grade(
                    a.required("--group")?,
                    a.optional("--split").unwrap_or("test"),
                    a.required("--task")?,
                    &evidence::read(a.path("--prediction")?)?,
                    a.path("--output")?,
                )
                .await;
        }
        "run" => {
            ensure(
                a.flag("--allow-model-requests"),
                "Execution requires --allow-model-requests",
            )?;
            let port = Harness::new(a.path("--auth")?, a.n("--run-reservation", 300000)?, 2400)?;
            let grader = gdpevo::Grader::new(dataset.clone(), a.required("--grading-image")?)?;
            return gdpevo::run(
                &dataset,
                a.required("--group")?,
                &port,
                &grader,
                a.path("--output")?,
                a.required("--environment-origin")?,
                a.n("--repeats", 3)?,
                a.n("--max-tokens", 2000000)?,
                a.flag("--owned-environment"),
            )
            .await;
        }
        _ => return Err(crate::Error::Invalid("Unknown GDPevo action".into())),
    };
    evidence::write(a.path("--output")?, &value)?;
    Ok(value)
}
