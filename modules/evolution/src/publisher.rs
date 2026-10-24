// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{contracts::Policy, package};
use exoharness_benchmark::{ManagementPort, Result, encoded, ensure, evidence, text};
use serde_json::{Value, json};
use std::path::Path;
pub struct Publisher<'a> {
    pub client: &'a dyn ManagementPort,
    pub policy: &'a Policy,
    path: String,
}
impl<'a> Publisher<'a> {
    pub fn new(client: &'a dyn ManagementPort, policy: &'a Policy) -> Self {
        Self {
            client,
            policy,
            path: format!("/v1/capabilities/{}", encoded(&policy.skill_id())),
        }
    }
    pub async fn record(&self) -> Result<Value> {
        self.client.api(&self.path, None, "GET", true).await
    }
    pub async fn existing(&self) -> Result<Value> {
        let record = self.record().await?;
        if !record.is_null() {
            let view = self
                .client
                .api(
                    &(self.path.clone() + "/skill-instructions"),
                    None,
                    "GET",
                    false,
                )
                .await?;
            ensure(
                view["revision"] == record["revision"],
                "Skill changed during read",
            )?;
            package::check_managed(&record, text(&view["content"])?, self.policy)?;
        }
        Ok(record)
    }
    pub async fn snapshot(&self) -> Result<Option<Value>> {
        let record = self.existing().await?;
        if record.is_null() {
            return Ok(None);
        }
        let exported = self
            .client
            .api(
                &format!(
                    "{}/skill-package?revision={}",
                    self.path, record["revision"]
                ),
                None,
                "GET",
                false,
            )
            .await?;
        ensure(
            exported["capability"] == record,
            "Immutable export differs from current Skill",
        )?;
        let body = json!({"version":record["manifest"]["version"],"scope":self.policy.value()["scope"],"permissions":[],"files":exported["files"]});
        package::validate_inventory(&body)?;
        ensure(
            package::package_digest(&body)? == record["source_digest"],
            "Package source identity mismatch",
        )?;
        let files = package::files(&body)?;
        let provenance: Value = serde_json::from_slice(&files[package::PROVENANCE])?;
        ensure(
            provenance["policy_id"] == self.policy.value()["id"],
            "Experience belongs to another policy",
        )?;
        Ok(Some(body))
    }
    pub async fn publish(
        &self,
        body: &Value,
        directory: &Path,
        expected_revision: &Value,
    ) -> Result<Value> {
        package::validate_inventory(body)?;
        ensure(
            body["scope"] == self.policy.value()["scope"]
                && body["permissions"].as_array().is_some_and(Vec::is_empty),
            "Publication cannot change authority",
        )?;
        let files = package::files(body)?;
        let instructions = std::str::from_utf8(&files["SKILL.md"])
            .map_err(|_| exoharness_benchmark::Error::Invalid("Invalid root encoding".into()))?;
        ensure(
            instructions.contains(&format!(
                "<!-- evolution-policy:{} -->",
                self.policy.s("id")
            )),
            "Candidate policy identity mismatch",
        )?;
        let _lock = evidence::Exclusive::acquire(directory)?;
        let path = directory.join("publication.json");
        let identity = json!({"skill_id":self.policy.skill_id(),"source_digest":package::package_digest(body)?,"expected_revision":expected_revision,"management_origin":self.client.origin()});
        let mut state = if path.exists() {
            evidence::read(&path)?
        } else {
            json!({"identity":identity,"phase":"planned"})
        };
        ensure(
            state["identity"] == identity,
            "Pending publication cannot change identity",
        )?;
        let mut record = self.existing().await?;
        if state["phase"] == "ready" {
            ensure(
                !record.is_null()
                    && record["revision"] == state["revision"]
                    && record["source_digest"] == identity["source_digest"],
                "Published Skill changed by another administrator",
            )?;
            return Ok(state);
        }
        if !record.is_null() && record["source_digest"] == identity["source_digest"] {
            ensure(
                ["importing", "imported", "preparing"]
                    .contains(&state["phase"].as_str().unwrap_or(""))
                    || record["revision"] == *expected_revision,
                "Unrelated revision cannot satisfy publication",
            )?;
        } else {
            ensure(
                record["revision"] == *expected_revision,
                "Skill revision conflict; no replacement",
            )?;
            ensure(
                state["phase"] != "importing",
                "Import outcome unknown; reconcile without resubmission",
            )?;
            state["phase"] = json!("importing");
            evidence::write(&path, &state)?;
            let mut payload = body.clone();
            if !expected_revision.is_null() {
                payload["expected_revision"] = expected_revision.clone();
            }
            let result = self
                .client
                .api("/v1/imports/skill", Some(&payload), "POST", false)
                .await?;
            record = result["capabilities"][0].clone();
            ensure(
                record["id"] == identity["skill_id"]
                    && record["source_digest"] == identity["source_digest"],
                "Import receipt does not match package",
            )?;
        }
        state["phase"] = json!("imported");
        state["revision"] = record["revision"].clone();
        evidence::write(&path, &state)?;
        let mut status = self
            .client
            .api(&(self.path.clone() + "/preparation"), None, "GET", false)
            .await?;
        if record["enabled"] != true || status["runtime_ready"] != true {
            state["phase"] = json!("preparing");
            evidence::write(&path, &state)?;
            status = self
                .client
                .api(
                    &(self.path.clone() + "/preparation"),
                    Some(&json!({"expected_revision":record["revision"],"confirm":true})),
                    "POST",
                    false,
                )
                .await?;
        }
        record = self.existing().await?;
        ensure(
            !record.is_null()
                && record["source_digest"] == identity["source_digest"]
                && status["runtime_ready"] == true,
            "Published Skill not ready; reconcile retained receipt",
        )?;
        state["phase"] = json!("ready");
        state["revision"] = record["revision"].clone();
        evidence::write(&path, &state)?;
        Ok(state)
    }
    pub async fn suspend(&self, revision: &Value, digest: &Value) -> Result<Value> {
        let record = self.existing().await?;
        ensure(
            !record.is_null()
                && record["revision"] == *revision
                && record["source_digest"] == *digest,
            "Refuse to suspend another Skill revision",
        )?;
        self.client
            .api(
                &(self.path.clone() + "/enabled"),
                Some(&json!({"enabled":false,"expected_revision":revision})),
                "PUT",
                false,
            )
            .await
    }
}
