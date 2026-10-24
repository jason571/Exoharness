// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{
    Result,
    contracts::{digest, sha256},
    encoded, ensure, evidence, list,
    metrics::usage,
    number, text,
    verifiers::inspect_artifact,
};
use async_trait::async_trait;
use serde_json::{Value, json};
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub fn origin(value: &str) -> Result<String> {
    let url = url::Url::parse(value)
        .map_err(|_| crate::Error::Invalid("Invalid evaluation origin".into()))?;
    ensure(
        ["https", "http"].contains(&url.scheme())
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.query().is_none()
            && url.fragment().is_none()
            && ["", "/"].contains(&url.path()),
        "Expected credential-free API origin",
    )?;
    ensure(
        url.scheme() == "https"
            || ["localhost", "127.0.0.1", "[::1]", "::1"].contains(&url.host_str().unwrap_or("")),
        "Remote plaintext API refused",
    )?;
    Ok(value.trim_end_matches('/').to_string())
}
pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
#[async_trait]
pub trait ManagementPort: Send + Sync {
    fn origin(&self) -> &str;
    fn runtime(&self) -> Result<String>;
    async fn api(
        &self,
        path: &str,
        payload: Option<&Value>,
        method: &str,
        optional: bool,
    ) -> Result<Value>;
    async fn configuration(&self, assistant: &str) -> Result<Value> {
        let caps = self.api("/v1/capabilities", None, "GET", false).await?;
        let evidence = capability_evidence(self, &caps).await?;
        Ok(
            json!({"profile":self.api(&format!("/v1/agent-profiles/{}",encoded(assistant)),None,"GET",false).await?,"models":self.api("/v1/model-catalog",None,"GET",false).await?,"capabilities":caps,"capability_evidence":evidence}),
        )
    }
}
async fn capability_evidence<M: ManagementPort + ?Sized>(port: &M, caps: &Value) -> Result<Value> {
    let mut records = list(caps)?.clone();
    records.sort_by_key(|v| v["id"].as_str().unwrap_or("").to_owned());
    let mut rows = Vec::new();
    for r in records {
        let path = format!("/v1/capabilities/{}", encoded(text(&r["id"])?));
        let mut permissions = list(
            &port
                .api(&(path.clone() + "/permissions"), None, "GET", false)
                .await?,
        )?
        .clone();
        permissions.sort_by_key(|p| p["permission"].as_str().unwrap_or("").to_owned());
        let mut row = json!({"id":r["id"],"permissions":permissions});
        if r["enabled"] == true && r["manifest"]["spec"]["kind"] == "mcp_server" {
            row["tools"] = port.api(&(path + "/tools"), None, "GET", false).await?;
        }
        rows.push(row);
    }
    Ok(json!(rows))
}
#[derive(Clone)]
pub struct Management {
    pub auth: Value,
    origin: String,
    client: reqwest::Client,
}
impl Management {
    pub fn new(path: &Path) -> Result<Self> {
        Self::from_auth(evidence::read_private(path)?)
    }
    pub fn from_auth(auth: Value) -> Result<Self> {
        let origin = origin(text(&auth["management_origin"])?)?;
        Ok(Self {
            auth,
            origin,
            client: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .retry(reqwest::retry::never())
                .timeout(Duration::from_secs(70))
                .build()?,
        })
    }
    async fn request(
        &self,
        base: &str,
        path: &str,
        headers: &Value,
        payload: Option<&Value>,
        method: &str,
        optional: bool,
    ) -> Result<Value> {
        ensure(
            path.starts_with('/') && !path.starts_with("//") && !path.contains('\\'),
            "Invalid API path",
        )?;
        let method = reqwest::Method::from_bytes(method.as_bytes())
            .map_err(|_| crate::Error::Invalid("Invalid HTTP method".into()))?;
        let mut request = self
            .client
            .request(method, format!("{base}{path}"))
            .header("Origin", base);
        for (k, v) in headers
            .as_object()
            .ok_or_else(|| crate::Error::Invalid("Missing private headers".into()))?
        {
            request = request.header(k, text(v)?);
        }
        if let Some(body) = payload {
            request = request.json(body);
        }
        let response = request.send().await?;
        if optional && response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(Value::Null);
        }
        ensure(
            response.status().is_success(),
            &format!(
                "Evaluation API returned HTTP {}",
                response.status().as_u16()
            ),
        )?;
        let raw = bounded(response, 32 * 1024 * 1024).await?;
        Ok(serde_json::from_slice(&raw)?)
    }
}
pub async fn bounded(mut response: reqwest::Response, maximum: usize) -> Result<Vec<u8>> {
    let mut raw = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure(
            raw.len().saturating_add(chunk.len()) <= maximum,
            "HTTP response exceeds evaluation bound",
        )?;
        raw.extend_from_slice(&chunk);
    }
    Ok(raw)
}
#[async_trait]
impl ManagementPort for Management {
    fn origin(&self) -> &str {
        &self.origin
    }
    fn runtime(&self) -> Result<String> {
        let binary = text(&self.auth["runtime_binary"])?;
        ensure(
            binary.starts_with("/proc/") && binary.ends_with("/exe"),
            "Pin the actual running service executable",
        )?;
        // /proc/<pid>/exe is intentionally a kernel-owned link, never an upload.
        let actual = sha256(&std::fs::read(binary)?);
        ensure(
            self.auth["runtime_version"] == actual,
            "Running service binary changed",
        )?;
        Ok(actual)
    }
    async fn api(
        &self,
        path: &str,
        payload: Option<&Value>,
        method: &str,
        optional: bool,
    ) -> Result<Value> {
        self.request(
            &self.origin,
            path,
            &self.auth["management_headers"],
            payload,
            method,
            optional,
        )
        .await
    }
}
#[async_trait]
pub trait BenchmarkHarness: Send + Sync {
    fn run_ceiling(&self) -> u64;
    fn user_id(&self) -> &str;
    fn portal(&self) -> &str;
    fn management(&self) -> &dyn ManagementPort;
    async fn configuration(&self) -> Result<Value>;
    async fn configuration_fingerprint(&self) -> Result<String> {
        Ok(digest(&self.configuration().await?))
    }
    async fn fingerprint(&self) -> Result<String>;
    async fn execute(
        &self,
        directory: &Path,
        prompts: &[String],
        fingerprint: &str,
        attachments: &[Value],
    ) -> Result<Value>;
}
pub struct Harness {
    pub management: Management,
    pub portal: String,
    artifact_origins: Vec<String>,
    user_id: String,
    run_ceiling: u64,
    timeout: u64,
}
impl Harness {
    pub fn new(path: &Path, run_ceiling: u64, timeout: u64) -> Result<Self> {
        let management = Management::new(path)?;
        let auth = &management.auth;
        let portal = origin(text(&auth["portal_origin"])?)?;
        let user = text(&auth["user_id"])?;
        ensure(
            auth["owned_test_account"] == true
                && user.starts_with("exo_eval_")
                && run_ceiling > 0
                && timeout > 0
                && run_ceiling >= number(&auth["run_token_ceiling"])?,
            "Use an owned test account and sufficient Run reservation",
        )?;
        let mut origins = vec![origin(text(&auth["artifact_origin"])?)?];
        if let Some(values) = auth.get("additional_artifact_origins") {
            ensure(list(values)?.len() <= 4, "Too many artifact origins")?;
            for value in list(values)? {
                origins.push(origin(text(value)?)?);
            }
        }
        let user_id = user.to_string();
        Ok(Self {
            management,
            portal,
            artifact_origins: origins,
            user_id,
            run_ceiling,
            timeout,
        })
    }
    pub async fn portal_api(&self, path: &str, payload: Option<&Value>) -> Result<Value> {
        self.management
            .request(
                &self.portal,
                path,
                &self.management.auth["portal_headers"],
                payload,
                if payload.is_some() { "POST" } else { "GET" },
                false,
            )
            .await
    }
    async fn artifact(
        &self,
        card: &Value,
        directory: &Path,
        index: usize,
    ) -> Result<Option<Value>> {
        let Some(address) = card["download_url"].as_str() else {
            return Ok(None);
        };
        let url = url::Url::parse(address)
            .map_err(|_| crate::Error::Invalid("Invalid artifact URL".into()))?;
        ensure(
            url.username().is_empty()
                && url.password().is_none()
                && url.fragment().is_none()
                && self
                    .artifact_origins
                    .contains(&origin(&url.origin().ascii_serialization())?),
            "Artifact origin outside allowlist",
        )?;
        let response = self
            .management
            .client
            .get(url)
            .timeout(Duration::from_secs(120))
            .send()
            .await?;
        ensure(response.status().is_success(), "Artifact download failed")?;
        let raw = bounded(response, 128 * 1024 * 1024).await?;
        let name = Path::new(card["filename"].as_str().unwrap_or("artifact"))
            .file_name()
            .and_then(|p| p.to_str())
            .unwrap_or("artifact");
        let ext = Path::new(name)
            .extension()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .to_lowercase();
        let path = directory.join(format!("artifact-{index}.{ext}"));
        evidence::write_bytes(&path, &raw)?;
        let probe = if ["mp4", "mp3", "wav"].contains(&ext.as_str()) {
            let mut command = tokio::process::Command::new("ffprobe");
            command
                .args([
                    "-v",
                    "error",
                    "-protocol_whitelist",
                    "file,pipe",
                    "-show_entries",
                    "format=duration:stream=codec_type",
                    "-of",
                    "json",
                ])
                .arg(&path)
                .kill_on_drop(true);
            let output = tokio::time::timeout(Duration::from_secs(30), command.output())
                .await
                .map_err(|_| crate::Error::Invalid("Artifact inspection timed out".into()))??;
            ensure(
                output.status.success() && output.stdout.len() <= 1024 * 1024,
                "Artifact inspection failed",
            )?;
            let p: Value = serde_json::from_slice(&output.stdout)?;
            let streams = list(&p["streams"])?;
            let duration = text(&p["format"]["duration"])?
                .parse::<f64>()
                .map_err(|_| crate::Error::Invalid("Invalid media duration".into()))?;
            ensure(
                duration.is_finite() && duration > 0.0,
                "Invalid media duration",
            )?;
            Some(
                json!({"duration":duration,"audio":streams.iter().any(|s|s["codec_type"]=="audio"),"video":streams.iter().any(|s|s["codec_type"]=="video")}),
            )
        } else {
            None
        };
        let mut value = inspect_artifact(name, &raw, probe)?;
        value["sha256"] = json!(sha256(&raw));
        Ok(Some(value))
    }
}
#[async_trait]
impl BenchmarkHarness for Harness {
    fn run_ceiling(&self) -> u64 {
        self.run_ceiling
    }
    fn user_id(&self) -> &str {
        &self.user_id
    }
    fn portal(&self) -> &str {
        &self.portal
    }
    fn management(&self) -> &dyn ManagementPort {
        &self.management
    }
    async fn configuration(&self) -> Result<Value> {
        self.management.runtime()?;
        let account = self.portal_api("/v1/account", None).await?;
        let account = account.get("account").unwrap_or(&account);
        ensure(account["id"] == self.user_id, "Test account changed")?;
        let profile = self.portal_api("/v1/agent-profile", None).await?;
        ensure(
            profile["id"] == self.management.auth["assistant_id"],
            "Test assistant changed",
        )?;
        let caps = self
            .management
            .api("/v1/capabilities", None, "GET", false)
            .await?;
        let evidence = capability_evidence(&self.management, &caps).await?;
        let models = self
            .management
            .api("/v1/model-catalog", None, "GET", false)
            .await?;
        Ok(
            json!({"profile":profile,"models":models,"capabilities":caps,"capability_evidence":evidence}),
        )
    }
    async fn fingerprint(&self) -> Result<String> {
        let mut value = self.configuration().await?;
        value["runtime_version"] = self.management.auth["runtime_version"].clone();
        Ok(digest(&value))
    }
    async fn execute(
        &self,
        directory: &Path,
        prompts: &[String],
        fingerprint: &str,
        attachments: &[Value],
    ) -> Result<Value> {
        evidence::private_dir(directory)?;
        let _lock = evidence::Exclusive::acquire(directory)?;
        let statepath = directory.join("execution.json");
        let resultpath = directory.join("outcome.json");
        let input = digest(&json!({"prompts":prompts,"attachments":attachments}));
        let mut state = if statepath.exists() {
            evidence::read(&statepath)?
        } else {
            json!({"turns":[],"input_sha256":input,"started_at_unix_ms":now_ms()})
        };
        ensure(
            state["input_sha256"] == input,
            "Saved evaluation input changed",
        )?;
        if resultpath.exists() {
            let result = evidence::read(&resultpath)?;
            ensure(
                result["runtime_fingerprint"] == fingerprint,
                "Cached outcome belongs to another runtime",
            )?;
            return Ok(result);
        }
        if state["session_id"].is_null() {
            ensure(
                state["session_intent"] != true,
                "Session submission unknown; do not resubmit",
            )?;
            state["session_intent"] = json!(true);
            evidence::write(&statepath, &state)?;
            let session=self.portal_api("/v1/sessions",Some(&json!({"title":format!("Offline evaluation {}",directory.file_name().unwrap_or_default().to_string_lossy())}))).await?;
            ensure(
                session["agent_profile"]["id"] == self.management.auth["assistant_id"],
                "New Session assistant mismatch",
            )?;
            state["session_id"] = session["id"].clone();
            evidence::write(&statepath, &state)?;
        }
        let sid = text(&state["session_id"])?.to_string();
        let (
            mut calls,
            mut tools,
            mut runs,
            mut states,
            mut reads,
            mut resources,
            mut successful,
            mut cards,
        ) = (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut answer = String::new();
        for (index, prompt) in prompts.iter().enumerate() {
            if list(&state["turns"])?.len() <= index {
                state["turns"]
                    .as_array_mut()
                    .expect("turns")
                    .push(json!({"intent":true,"prompt_sha256":digest(&json!(prompt))}));
                evidence::write(&statepath, &state)?;
                let mut payload = json!({"prompt":prompt});
                if index == 0 && !attachments.is_empty() {
                    payload["attachments"] = json!(attachments);
                }
                let submitted = self
                    .portal_api(
                        &format!("/v1/sessions/{}/turns", encoded(&sid)),
                        Some(&payload),
                    )
                    .await?;
                state["turns"][index]["run_id"] = submitted["run_id"].clone();
                evidence::write(&statepath, &state)?;
            }
            let turn = &state["turns"][index];
            ensure(
                turn["prompt_sha256"] == digest(&json!(prompt)) && !turn["run_id"].is_null(),
                "Turn submission unknown or changed; do not resubmit",
            )?;
            let rid = text(&turn["run_id"])?.to_string();
            runs.push(rid.clone());
            let deadline = tokio::time::Instant::now() + Duration::from_secs(self.timeout);
            let trace = loop {
                let trace = self
                    .management
                    .api(
                        &format!("/v1/trace/runs/{}?event_limit=1", encoded(&rid)),
                        None,
                        "GET",
                        false,
                    )
                    .await?;
                let status = text(&trace["run"]["status"])?;
                let measured = usage(list(&trace["model_calls"])?);
                if !["running", "queued", "pending", "waiting_authorization"].contains(&status) {
                    break trace;
                }
                if tokio::time::Instant::now() >= deadline
                    || number(&measured["total_tokens"])? >= self.run_ceiling
                {
                    self.portal_api(
                        &format!("/v1/runs/{}/cancel", encoded(&rid)),
                        Some(&json!({})),
                    )
                    .await?;
                    return Err(crate::Error::Invalid("Evaluation observation limit reached; inspect retained Run without replacement".into()));
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            };
            evidence::write(&directory.join(format!("trace-{index}.json")), &trace)?;
            calls.extend(list(&trace["model_calls"])?.iter().cloned());
            for i in list(&trace["invocations"])? {
                let name = text(&i["tool_name"])?;
                tools.push(name.to_string());
                let r = &i["result"];
                if r["_host_execution"] == true
                    && !crate::flagged(&r["isError"])
                    && !crate::flagged(&r["is_error"])
                    && i["outcome_status"] == "success"
                {
                    successful.push(name.to_string());
                }
                if name == "read_skill"
                    && r["content"].as_str().is_some_and(|s| !s.is_empty())
                    && !crate::flagged(&r["isError"])
                {
                    reads.push(
                        json!({"id":i["arguments"]["id"],"revision":r["revision"],"run_id":rid}),
                    );
                    if r["path"] == "resource-index.json" {
                        resources.push(json!({"id":i["arguments"]["id"],"revision":r["revision"],"run_id":rid,"resources":r.get("selected_resources").cloned().unwrap_or(json!([]))}));
                    }
                }
            }
            states.push(text(&trace["run"]["status"])?.to_string());
            let history = self
                .portal_api(
                    &format!("/v1/sessions/{}?turn_limit=8", encoded(&sid)),
                    None,
                )
                .await?;
            let turns = list(&history["turns"])?;
            let saved = turns
                .iter()
                .find(|t| t["run_id"] == rid)
                .or_else(|| {
                    turns.iter().find(|t| {
                        t["run_ids"]
                            .as_array()
                            .is_some_and(|rs| rs.contains(&json!(rid)))
                    })
                })
                .ok_or_else(|| {
                    crate::Error::Invalid("Evaluated Turn missing from history".into())
                })?;
            let response = &saved["response"];
            answer = response["answer"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| response["text"].as_str().filter(|s| !s.is_empty()))
                .or_else(|| saved["answer"].as_str().filter(|s| !s.is_empty()))
                .unwrap_or("")
                .to_string();
            if let Some(a) = response["artifacts"].as_array() {
                cards.extend(a.iter().cloned());
            }
        }
        let measured = usage(&calls);
        let (mut artifacts, mut errors, mut seen) =
            (Vec::new(), Vec::new(), std::collections::BTreeSet::new());
        for card in cards {
            let key = card
                .get("artifact_id")
                .filter(|value| crate::flagged(value))
                .or_else(|| {
                    card.get("download_url")
                        .filter(|value| crate::flagged(value))
                })
                .cloned()
                .unwrap_or(Value::Null);
            if !key.is_null() && seen.insert(key.to_string()) {
                match self.artifact(&card, directory, artifacts.len()).await {
                    Ok(Some(value)) => artifacts.push(value),
                    Ok(None) => {}
                    Err(_) => {
                        errors.push(json!({"index":seen.len()-1,"code":"ArtifactInspectionFailed"}))
                    }
                }
            }
        }
        let bindings: std::collections::BTreeSet<_> = calls
            .iter()
            .map(|c| json!([c["provider"], c["model"]]).to_string())
            .collect();
        let bindings: Vec<Value> = bindings
            .iter()
            .map(|b| serde_json::from_str(b))
            .collect::<std::result::Result<_, _>>()?;
        let duration = calls
            .iter()
            .map(|c| c["duration_ms"].as_u64())
            .collect::<Option<Vec<_>>>()
            .map(|v| v.iter().sum::<u64>());
        let result = json!({"state":states.last(),"turn_states":states,"text":answer,"artifacts":artifacts,"run_ids":runs,"tool_names":tools,"successful_tool_names":successful,"artifact_errors":errors,"usage":measured,"tokens":if measured["unknown_model_calls"]==0{Some(number(&measured["total_tokens"])?) }else{None},"runtime_fingerprint":fingerprint,"runtime_drift":self.fingerprint().await?!=fingerprint,"model_bindings":bindings,"skill_reads":reads,"skill_resource_reads":resources,"elapsed_ms":now_ms().saturating_sub(state["started_at_unix_ms"].as_u64().unwrap_or(now_ms())),"model_duration_ms":duration});
        evidence::write(&resultpath, &result)?;
        Ok(result)
    }
}
