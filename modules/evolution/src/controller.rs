// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! Train, validate, hold out and publish with a persistent cumulative budget.
use crate::{
    contracts::Policy,
    evaluation, learning,
    ledger::{Ledger, consume_holdout},
    package,
    publisher::Publisher,
    trajectories::Trajectories,
};
use exoharness_benchmark::{
    BenchmarkHarness, ManagementPort, Result, Suite,
    campaign::{self, Options},
    digest, ensure, evidence, list, text, verifiers,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
pub struct Environment {
    policy: Policy,
    suite: Suite,
    baseline: exoharness_benchmark::Harness,
    canary: exoharness_benchmark::Harness,
    production: exoharness_benchmark::Management,
}
impl Environment {
    pub fn load(job: &crate::contracts::Job) -> Result<Self> {
        let policy = Policy::parse(evidence::read_private(&job.policy)?)?;
        let suite = Suite::parse(evidence::read(&job.suite)?)?;
        let baseline = exoharness_benchmark::Harness::new(
            &job.baseline_auth,
            policy.n("run_reservation"),
            2400,
        )?;
        let canary = exoharness_benchmark::Harness::new(
            &job.canary_auth,
            policy.n("run_reservation"),
            2400,
        )?;
        let production = exoharness_benchmark::Management::new(&job.publication_auth)?;
        Ok(Self {
            policy,
            suite,
            baseline,
            canary,
            production,
        })
    }
    pub fn controller(&self, job: &crate::contracts::Job, root: &Path) -> Result<Controller<'_>> {
        Controller::new(
            &self.policy,
            &self.suite,
            &self.baseline,
            &self.canary,
            &self.production,
            root,
            Some(&job.input_root),
            job.training_evidence.as_deref(),
        )
    }
}
pub struct Controller<'a> {
    pub policy: &'a Policy,
    pub suite: &'a Suite,
    pub baseline: &'a dyn BenchmarkHarness,
    pub canary: &'a dyn BenchmarkHarness,
    pub production: Publisher<'a>,
    pub staging: Publisher<'a>,
    pub root: PathBuf,
    pub input_root: Option<PathBuf>,
    pub trajectories: Trajectories,
}
impl<'a> Controller<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        policy: &'a Policy,
        suite: &'a Suite,
        baseline: &'a dyn BenchmarkHarness,
        canary: &'a dyn BenchmarkHarness,
        production: &'a dyn ManagementPort,
        root: &Path,
        input_root: Option<&Path>,
        training_evidence: Option<&Path>,
    ) -> Result<Self> {
        ensure(
            suite.sha256() == policy.s("suite_sha256"),
            "Suite differs from frozen policy",
        )?;
        for split in ["validation", "test"] {
            evaluation::phase_suite(suite, policy.s("group"), split)?;
        }
        let owner = policy.value()["scope"]["user_id"].as_str();
        let trajectories = Trajectories::load(training_evidence, suite, policy.s("group"), owner)?;
        ensure(
            policy.method() != "workflow" || !trajectories.rows.is_empty(),
            "Workflow induction requires reviewed evidence",
        )?;
        Ok(Self {
            policy,
            suite,
            baseline,
            canary,
            production: Publisher::new(production, policy),
            staging: Publisher::new(canary.management(), policy),
            root: root.into(),
            input_root: input_root.map(ToOwned::to_owned),
            trajectories,
        })
    }
    async fn open(&self) -> Result<Ledger> {
        let configuration = evaluation::check_environment(
            self.policy,
            self.baseline,
            self.canary,
            self.production.client,
        )
        .await?;
        let mut identity = json!({"policy_sha256":self.policy.sha256(),"suite_sha256":self.suite.sha256(),"baseline_binary_sha256":self.policy.value()["baseline_binary_sha256"],"production_origin":self.production.client.origin(),"baseline_origin":self.baseline.portal(),"canary_origin":self.canary.portal(),"configuration_sha256":configuration});
        if self.policy.method() == "workflow" {
            identity["training_evidence_sha256"] = json!(self.trajectories.identity);
            identity["learning_skill_sha256"] = json!(exoharness_benchmark::contracts::sha256(
                package::INSTRUCTIONS.as_bytes()
            ));
        }
        Ledger::open(&self.root, identity)
    }
    fn options(&self) -> Options {
        Options {
            repeats: self.policy.n("repeats"),
            max_tokens: self.policy.n("max_tokens"),
            run_reservation: self.policy.n("run_reservation"),
            input_root: self.input_root.clone(),
        }
    }
    async fn evaluate(
        &self,
        ledger: &mut Ledger,
        index: usize,
        split: &str,
        learning_tokens: u64,
    ) -> Result<Value> {
        let round = ledger.state["rounds"][index].clone();
        let id = text(&round["id"])?;
        let key = format!("{id}-{split}");
        if ledger.state["phase_limits"].is_null() {
            ledger.state["phase_limits"] = json!({});
        }
        if ledger.state["phase_limits"].get(&key).is_none() {
            let remaining = self
                .policy
                .n("max_tokens")
                .checked_sub(ledger.charged()?)
                .ok_or_else(|| {
                    exoharness_benchmark::Error::Invalid("Evolution budget exhausted".into())
                })?;
            ensure(
                remaining >= self.policy.n("run_reservation"),
                "Evolution cannot admit evaluation",
            )?;
            ledger.state["phase_limits"][&key] = json!(remaining);
            ledger.state["charges"][&key] = json!(remaining);
            ledger.save()?;
        }
        let body = evidence::read(&self.root.join(id).join("package.json"))?;
        let files = package::files(&body)?;
        let resource_index = files
            .get("resource-index.json")
            .map(|raw| serde_json::from_slice::<Value>(raw))
            .transpose()?;
        let output = self.root.join(&key);
        let phase = evaluation::Phase {
            split,
            learning_tokens,
            max_tokens: ledger.state["phase_limits"][&key]
                .as_u64()
                .expect("phase budget"),
            revision: &round["canary_revision"],
            resource_index: resource_index.as_ref(),
        };
        let result = evaluation::compare(
            self.policy,
            self.suite,
            self.baseline,
            self.canary,
            &output,
            self.input_root.as_deref(),
            phase,
        )
        .await;
        let statefile = output.join("comparison.json");
        if statefile.exists() {
            ledger.state["charges"][&key] = json!(campaign::charges(&evidence::read(&statefile)?)?);
            ledger.save()?;
        }
        result
    }
    async fn training_feedback(
        &self,
        ledger: &mut Ledger,
        id: &str,
        revision: &Value,
        body: &Value,
    ) -> Result<Vec<Value>> {
        let mut feedback = Vec::new();
        let files = package::files(body)?;
        let resource_index = files
            .get("resource-index.json")
            .map(|raw| serde_json::from_slice::<Value>(raw))
            .transpose()?;
        for task in self
            .suite
            .tasks()
            .iter()
            .filter(|t| t["split"] == "train" && t["group"] == self.policy.value()["group"])
        {
            let inputs = campaign::attachments(task, self.input_root.as_deref())?;
            let turns = campaign::prompts(task)?;
            let fingerprint = self.canary.fingerprint().await?;
            let outcome = ledger
                .execute(
                    &format!("{id}/train-{}", text(&task["id"])?),
                    self.canary,
                    &turns,
                    &fingerprint,
                    &self.options(),
                    &inputs,
                )
                .await?;
            let verified = verifiers::verify(task, &outcome);
            let reuse = evaluation::reuse_failures(
                self.policy,
                &outcome,
                revision,
                resource_index.as_ref(),
            )?;
            let mut checks = list(&verified["checks"])?.clone();
            checks
                .push(json!({"kind":"experience_reuse","passed":reuse.is_empty(),"reasons":reuse}));
            feedback.push(json!({"train_id":task["id"],"passed":verified["delivered"]==true&&reuse.is_empty(),"checks":checks,"answer":text(&outcome["text"])?.chars().take(2000).collect::<String>()}));
        }
        evidence::write(
            &self.root.join(id).join("train-feedback.json"),
            &json!(feedback),
        )?;
        Ok(feedback)
    }
    pub async fn run(&self) -> Result<Value> {
        let _lock = evidence::Exclusive::acquire(&self.root)?;
        let mut ledger = self.open().await?;
        if ledger.state["phase"] == "validated" && self.policy.value()["publish"] == true {
            return self.publish_ledger(&mut ledger).await;
        }
        if [
            "published",
            "validated",
            "validation_rejected",
            "test_rejected",
            "unchanged",
            "exhausted",
        ]
        .contains(&ledger.state["phase"].as_str().unwrap_or(""))
        {
            return Ok(ledger.state);
        }
        if ledger.state.get("initial_revision").is_none() {
            let current = self.production.existing().await?;
            let config = self.baseline.configuration().await?;
            let baseline = list(&config["capabilities"])?
                .iter()
                .find(|r| r["id"] == self.policy.skill_id())
                .cloned()
                .unwrap_or(Value::Null);
            ensure(
                baseline["source_digest"] == current["source_digest"]
                    && baseline["enabled"] == current["enabled"],
                "Baseline is not the actual production incumbent",
            )?;
            ledger.state["initial_revision"] = current["revision"].clone();
            if let Some(body) = self.production.snapshot().await? {
                evidence::write(&self.root.join("incumbent-package.json"), &body)?;
                ledger.state["incumbent_digest"] = json!(package::package_digest(&body)?);
            }
            ledger.save()?;
        }
        ensure(
            self.production.existing().await?["revision"] == ledger.state["initial_revision"],
            "Production experience changed during campaign",
        )?;
        let rounds = list(&ledger.state["rounds"])?;
        let start = if rounds.last().is_some_and(|r| r["phase"] != "rejected") {
            rounds.len() - 1
        } else {
            rounds.len()
        };
        for index in start..self.policy.n("max_rounds") as usize {
            if list(&ledger.state["rounds"])?.len() == index {
                ledger.state["rounds"]
                    .as_array_mut()
                    .expect("rounds")
                    .push(json!({"id":format!("round-{index}"),"phase":"learning"}));
                ledger.save()?;
            }
            let id = text(&ledger.state["rounds"][index]["id"])?.to_string();
            let bodypath = self.root.join(&id).join("package.json");
            if !bodypath.exists() {
                let mut incumbent = Value::Null;
                let mut feedback = Vec::new();
                if index > 0 {
                    let previous = self.root.join(format!("round-{}", index - 1));
                    if previous.join("experience.json").exists() {
                        incumbent = evidence::read(&previous.join("experience.json"))?;
                    }
                    if previous.join("train-feedback.json").exists() {
                        feedback =
                            list(&evidence::read(&previous.join("train-feedback.json"))?)?.clone();
                    }
                } else if self.root.join("incumbent-package.json").exists() {
                    let old = package::files(&evidence::read(
                        &self.root.join("incumbent-package.json"),
                    )?)?;
                    incumbent = serde_json::from_slice(&old[package::EXPERIENCE])?;
                }
                let (candidate, runs, corrections) = learning::propose(
                    self.policy,
                    self.suite,
                    &mut ledger,
                    self.baseline,
                    &id,
                    &incumbent,
                    &feedback,
                    &self.trajectories,
                )
                .await?;
                evidence::write(
                    &self.root.join(&id).join("candidate-validation.json"),
                    &json!({"corrections":corrections,"run_ids":runs}),
                )?;
                evidence::write(&self.root.join(&id).join("experience.json"), &candidate)?;
                let hash = digest(&candidate);
                if list(&ledger.state["tested_candidates"])?.contains(&json!(hash)) {
                    ledger.state["rounds"][index]["phase"] = json!("rejected");
                    ledger.state["rounds"][index]["reason"] = json!("unchanged_candidate");
                    ledger.state["phase"] = json!("unchanged");
                    ledger.save()?;
                    return Ok(ledger.state);
                }
                ledger.state["tested_candidates"]
                    .as_array_mut()
                    .expect("tested candidates")
                    .push(json!(hash));
                let body = package::compile(
                    self.policy,
                    self.suite,
                    &candidate,
                    &id,
                    &runs,
                    &self.trajectories,
                )?;
                evidence::write(&bodypath, &body)?;
                ledger.state["rounds"][index]["candidate_sha256"] = json!(hash);
                ledger.state["rounds"][index]["canary_expected_revision"] =
                    self.staging.existing().await?["revision"].clone();
                ledger.save()?;
            }
            let body = evidence::read(&bodypath)?;
            if ledger.state["rounds"][index]
                .get("canary_expected_revision")
                .is_none()
            {
                let candidate = evidence::read(&self.root.join(&id).join("experience.json"))?;
                ledger.state["rounds"][index]["candidate_sha256"] = json!(digest(&candidate));
                ledger.state["rounds"][index]["canary_expected_revision"] =
                    self.staging.existing().await?["revision"].clone();
                ledger.save()?;
            }
            let receipt = self
                .staging
                .publish(
                    &body,
                    &self.root.join(&id).join("canary-publication"),
                    &ledger.state["rounds"][index]["canary_expected_revision"],
                )
                .await?;
            ledger.state["rounds"][index]["canary_revision"] = receipt["revision"].clone();
            ledger.save()?;
            if self.policy.s("supervision") != "self"
                && !self
                    .training_feedback(&mut ledger, &id, &receipt["revision"], &body)
                    .await?
                    .iter()
                    .all(|f| f["passed"] == true)
            {
                ledger.state["rounds"][index]["phase"] = json!("rejected");
                ledger.state["rounds"][index]["reason"] = json!("train_feedback");
                ledger.save()?;
                continue;
            }
            let learning_tokens = ledger.state["charges"]
                .as_object()
                .expect("charges")
                .iter()
                .filter(|(k, _)| k.contains("/learner") || k.contains("/train-"))
                .map(|(_, v)| v.as_u64().unwrap_or(0))
                .sum();
            let validation = self
                .evaluate(&mut ledger, index, "validation", learning_tokens)
                .await?;
            if list(&validation["decision"]["reasons"])?.contains(&json!("delivery_review_pending"))
            {
                ledger.state["rounds"][index]["phase"] = json!("awaiting_validation_review");
                ledger.save()?;
                return Ok(ledger.state);
            }
            if validation["decision"]["retain"] != true {
                ledger.state["rounds"][index]["phase"] = json!("rejected");
                ledger.state["rounds"][index]["reason"] = validation["decision"]["reasons"].clone();
                ledger.state["phase"] = json!("validation_rejected");
                ledger.save()?;
                return Ok(ledger.state);
            }
            consume_holdout(
                &self
                    .root
                    .parent()
                    .expect("campaign parent")
                    .join(".evolution-holdouts"),
                self.suite,
                self.policy.s("group"),
                text(&ledger.state["rounds"][index]["candidate_sha256"])?,
            )?;
            let search_cost = learning_tokens
                + list(&validation["rows"])?
                    .iter()
                    .filter(|r| r["mode"] == "candidate")
                    .map(|r| r["tokens"].as_u64().unwrap_or(0))
                    .sum::<u64>();
            let final_report = self
                .evaluate(&mut ledger, index, "test", search_cost)
                .await?;
            if list(&final_report["decision"]["reasons"])?
                .contains(&json!("delivery_review_pending"))
            {
                ledger.state["rounds"][index]["phase"] = json!("awaiting_test_review");
                ledger.save()?;
                return Ok(ledger.state);
            }
            if final_report["decision"]["retain"] != true {
                ledger.state["rounds"][index]["phase"] = json!("test_rejected");
                ledger.state["rounds"][index]["reason"] =
                    final_report["decision"]["reasons"].clone();
                ledger.state["phase"] = json!("test_rejected");
                ledger.save()?;
                return Ok(ledger.state);
            }
            ledger.state["rounds"][index]["phase"] = json!("validated");
            ledger.state["phase"] = json!("validated");
            evidence::write(
                &self.root.join("release.json"),
                &json!({"policy_sha256":self.policy.sha256(),"candidate_sha256":ledger.state["rounds"][index]["candidate_sha256"],"package_sha256":package::package_digest(&body)?,"validation_report_sha256":digest(&validation),"test_report_sha256":digest(&final_report),"expected_revision":ledger.state["initial_revision"],"round_id":id}),
            )?;
            ledger.save()?;
            if self.policy.value()["publish"] == true {
                return self.publish_ledger(&mut ledger).await;
            }
            return Ok(ledger.state);
        }
        ledger.state["phase"] = json!("exhausted");
        ledger.save()?;
        Ok(ledger.state)
    }
    pub async fn publish(&self) -> Result<Value> {
        let _lock = evidence::Exclusive::acquire(&self.root)?;
        let mut ledger = self.open().await?;
        self.publish_ledger(&mut ledger).await
    }
    async fn publish_ledger(&self, ledger: &mut Ledger) -> Result<Value> {
        if ledger.state["phase"] == "published" {
            return Ok(ledger.state.clone());
        }
        ensure(
            ledger.state["phase"] == "validated",
            "Only a fully validated candidate can publish",
        )?;
        let release = evidence::read(&self.root.join("release.json"))?;
        let id = text(&release["round_id"])?;
        let body = evidence::read(&self.root.join(id).join("package.json"))?;
        for split in ["validation", "test"] {
            let report = evidence::read(
                &self
                    .root
                    .join(format!("{id}-{split}"))
                    .join("evolution-report.json"),
            )?;
            ensure(
                digest(&report) == release[format!("{split}_report_sha256")]
                    && report["decision"]["retain"] == true,
                "Release evidence changed or rejected",
            )?;
        }
        ensure(
            release["policy_sha256"] == self.policy.sha256()
                && release["package_sha256"] == package::package_digest(&body)?
                && self.production.client.runtime()? == self.policy.s("baseline_binary_sha256"),
            "Release package, policy or runtime changed",
        )?;
        let mut receipt = self
            .production
            .publish(
                &body,
                &self.root.join("production-publication"),
                &release["expected_revision"],
            )
            .await?;
        receipt["candidate_sha256"] = release["candidate_sha256"].clone();
        ledger.state["publications"]
            .as_array_mut()
            .expect("publications")
            .push(receipt);
        ledger.state["phase"] = json!("published");
        ledger.save()?;
        Ok(ledger.state.clone())
    }
    pub async fn monitor(&self) -> Result<Value> {
        let _lock = evidence::Exclusive::acquire(&self.root)?;
        let saved = evidence::read(&self.root.join("evolution.json"))?;
        let mut ledger = Ledger::open(&self.root, saved["identity"].clone())?;
        ensure(
            ledger.state["identity"]["policy_sha256"] == self.policy.sha256()
                && ledger.state["identity"]["production_origin"] == self.production.client.origin(),
            "Monitoring policy differs from campaign",
        )?;
        self.production.client.runtime()?;
        crate::monitor::observe(&self.production, &mut ledger).await
    }
}
