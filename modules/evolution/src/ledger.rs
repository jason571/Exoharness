// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use exoharness_benchmark::{
    BenchmarkHarness, Result, Suite,
    campaign::{self, Options},
    digest, ensure, evidence,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub struct Ledger {
    pub root: PathBuf,
    pub state: Value,
}
impl Ledger {
    pub fn open(root: &Path, identity: Value) -> Result<Self> {
        evidence::private_dir(root)?;
        let path = root.join("evolution.json");
        let state = if path.exists() {
            evidence::read(&path)?
        } else {
            json!({"identity":identity,"phase":"learning","charges":{},"rounds":[],"tested_candidates":[],"publications":[],"monitor":{}})
        };
        ensure(
            state["identity"] == identity,
            "Evolution identity changed; use a new campaign",
        )?;
        let ledger = Self {
            root: root.into(),
            state,
        };
        ledger.save()?;
        Ok(ledger)
    }
    pub fn save(&self) -> Result<()> {
        evidence::write(&self.root.join("evolution.json"), &self.state)
    }
    pub fn charged(&self) -> Result<u64> {
        campaign::charges(&self.state)
    }
    pub async fn execute(
        &mut self,
        key: &str,
        port: &dyn BenchmarkHarness,
        prompts: &[String],
        fingerprint: &str,
        options: &Options,
        attachments: &[Value],
    ) -> Result<Value> {
        campaign::execute_charged(
            &mut self.state,
            &self.root.join("evolution.json"),
            key,
            port,
            prompts,
            fingerprint,
            attachments,
            options,
        )
        .await
    }
}
pub fn consume_holdout(root: &Path, suite: &Suite, group: &str, candidate: &str) -> Result<()> {
    let _lock = evidence::Exclusive::acquire(root)?;
    let path = root.join("holdouts.json");
    let mut state = if path.exists() {
        evidence::read(&path)?
    } else {
        json!({})
    };
    for task in suite
        .tasks()
        .iter()
        .filter(|t| t["split"] == "test" && t["group"] == group)
    {
        let key = digest(
            &json!({"turns":task["turns"],"inputs":task.get("attachments").cloned().unwrap_or(json!([]))}),
        );
        ensure(
            state.get(&key).is_none_or(|s| s == candidate),
            "Held-out input already judged a different candidate",
        )?;
        state[&key] = json!(candidate);
    }
    evidence::write(&path, &state)
}
