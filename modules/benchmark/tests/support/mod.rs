#![allow(dead_code)]
use axum::{
    Router,
    extract::{Request, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_benchmark::{contracts::sha256, evidence};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
pub struct StateData {
    pub namespace: u64,
    pub histories: std::collections::BTreeMap<u64, Value>,
    pub sessions: u64,
    pub turns: u64,
    pub imports: u64,
    pub amount: u64,
    pub training_amounts: Vec<u64>,
    pub learner_candidates: Vec<Value>,
    pub learner_prompts: Vec<String>,
    pub tokens: u64,
    pub lost_session: bool,
    pub lost_turn: bool,
    pub lost_import: bool,
    pub invalid_learner: bool,
    pub unknown_usage: bool,
    pub wrong_read_revision: bool,
    pub learner_uses_tool: bool,
    pub omit_resource_read: bool,
    pub artifact_redirect: bool,
    pub download_artifact: bool,
    pub artifact_id: Value,
    pub response_override: Option<Value>,
    pub saved_answer: Option<String>,
    pub record: Value,
    pub files: Value,
    pub candidate: Value,
    pub authenticated_download: bool,
    pub trace_sessions: Vec<Value>,
    pub trace_runs: Vec<Value>,
    pub trace_records: std::collections::BTreeMap<String, Value>,
}
pub struct Fixture {
    pub origin: String,
    pub state: Arc<Mutex<StateData>>,
    task: tokio::task::JoinHandle<()>,
}
impl Fixture {
    pub async fn start(tokens: u64, candidate: Value) -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let state = Arc::new(Mutex::new(StateData {
            namespace: NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            histories: Default::default(),
            sessions: 0,
            turns: 0,
            imports: 0,
            amount: 42,
            training_amounts: Vec::new(),
            learner_candidates: Vec::new(),
            learner_prompts: Vec::new(),
            tokens,
            lost_session: false,
            lost_turn: false,
            lost_import: false,
            invalid_learner: false,
            unknown_usage: false,
            wrong_read_revision: false,
            learner_uses_tool: false,
            omit_resource_read: false,
            artifact_redirect: false,
            download_artifact: false,
            artifact_id: Value::Null,
            response_override: None,
            saved_answer: None,
            record: Value::Null,
            files: json!([]),
            candidate,
            authenticated_download: false,
            trace_sessions: Vec::new(),
            trace_runs: Vec::new(),
            trace_records: Default::default(),
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new().fallback(handle).with_state(state.clone());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            origin,
            state,
            task,
        }
    }
    pub fn auth(&self, path: &Path) -> Value {
        let binary = format!("/proc/{}/exe", std::process::id());
        let hash = sha256(&std::fs::read(&binary).unwrap());
        let value = json!({"management_origin":self.origin,"portal_origin":self.origin,"artifact_origin":self.origin,"portal_headers":{"Cookie":"owned-test-cookie"},"management_headers":{"Authorization":"Bearer owned-test-key"},"user_id":"exo_eval_fixture","assistant_id":"fixture","owned_test_account":true,"runtime_binary":binary,"runtime_version":hash,"run_token_ceiling":1000});
        evidence::write(path, &value).unwrap();
        value
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn reply(value: Value) -> Response {
    axum::Json(value).into_response()
}
async fn handle(State(shared): State<Arc<Mutex<StateData>>>, request: Request) -> Response {
    let raw = request.uri().to_string();
    let rawpath = raw.split('?').next().unwrap();
    let path = percent_encoding::percent_decode_str(rawpath)
        .decode_utf8_lossy()
        .to_string();
    let method = request.method().to_string();
    let host = request
        .headers()
        .get("Host")
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let authenticated = request.headers().get("Authorization").is_some()
        || request.headers().get("Cookie").is_some();
    let bytes = axum::body::to_bytes(request.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let payload = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    let mut state = shared.lock().unwrap();
    if path.starts_with("/download/") {
        state.authenticated_download |= authenticated;
        if state.artifact_redirect {
            return (StatusCode::FOUND, [("Location", "/must-not-follow")]).into_response();
        }
        return (
            [("Content-Type", "text/html")],
            "<!doctype html><html>Rust</html>",
        )
            .into_response();
    }
    if !authenticated {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let profile = json!({"id":"fixture","active_revision":1,"access":{"skills":{"mode":"all"}}});
    let route = if path.starts_with("/v1/sessions/session-") {
        if path.ends_with("/turns") {
            "/fixture/turns"
        } else {
            "/fixture/session"
        }
    } else {
        path.as_str()
    };
    match (route, method.as_str()) {
        ("/v1/account", "GET") => reply(json!({"id":"exo_eval_fixture"})),
        ("/v1/agent-profile" | "/v1/agent-profiles/fixture", "GET") => reply(profile),
        ("/v1/model-catalog", "GET") => reply(json!({"models":[{"id":"fixture"}]})),
        ("/v1/capabilities", "GET") => reply(if state.record.is_null() {
            json!([])
        } else {
            json!([state.record])
        }),
        ("/v1/trace/sessions", "GET") => reply(json!(state.trace_sessions)),
        ("/v1/trace/runs", "GET") => reply(json!(state.trace_runs)),
        ("/v1/sessions", "POST") => {
            state.sessions += 1;
            if state.lost_session {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            reply(json!({"id":format!("session-{}",state.namespace),"agent_profile":profile}))
        }
        ("/fixture/turns", "POST") => {
            state.turns += 1;
            let turn = state.turns;
            if state.lost_turn {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            let learner = payload["prompt"]
                .as_str()
                .unwrap_or("")
                .contains("TRAIN MATERIAL");
            let answer = if learner {
                state
                    .learner_prompts
                    .push(payload["prompt"].as_str().unwrap().to_string());
                if state.invalid_learner {
                    "invalid".to_string()
                } else if !state.learner_candidates.is_empty() {
                    state.learner_candidates.remove(0).to_string()
                } else {
                    state.candidate.to_string()
                }
            } else {
                let amount = if payload["prompt"]
                    .as_str()
                    .is_some_and(|p| p.contains("Compute training amount"))
                {
                    state
                        .training_amounts
                        .get(state.imports.saturating_sub(1) as usize)
                        .copied()
                        .unwrap_or(state.amount)
                } else {
                    state.amount
                };
                json!({"amount":amount,"product":312}).to_string()
            };
            let rid = format!("run-{}-{turn}", state.namespace);
            let artifacts = if state.artifact_redirect {
                json!([{"filename":"page.html","download_url":format!("http://{host}/download/redirect.html")}])
            } else if state.download_artifact {
                let card = json!({"artifact_id":state.artifact_id,"filename":"page.html","download_url":format!("http://{host}/download/page.html")});
                json!([card, card])
            } else {
                json!([])
            };
            let response = state
                .response_override
                .clone()
                .unwrap_or_else(|| json!({"answer":answer,"artifacts":artifacts}));
            let mut history = json!({"learner":learner,"run_id":rid,"response":response});
            if let Some(answer) = &state.saved_answer {
                history["answer"] = json!(answer);
            }
            state.histories.insert(turn, history);
            reply(json!({"run_id":rid}))
        }
        ("/fixture/session", "GET") => {
            reply(json!({"turns":state.histories.values().collect::<Vec<_>>()}))
        }
        ("/v1/imports/skill", "POST") => {
            state.imports += 1;
            let root = payload["files"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| f["path"] == "SKILL.md")
                .unwrap();
            let root = String::from_utf8(
                STANDARD
                    .decode(root["content_base64"].as_str().unwrap())
                    .unwrap(),
            )
            .unwrap();
            let name = root.lines().find_map(|l| l.strip_prefix("name: ")).unwrap();
            let revision = state.record["revision"].as_u64().unwrap_or(0) + 1;
            if !state.record.is_null() && payload["expected_revision"] != state.record["revision"] {
                return StatusCode::CONFLICT.into_response();
            }
            let mut files = std::collections::BTreeMap::new();
            for f in payload["files"].as_array().unwrap() {
                files.insert(
                    f["path"].as_str().unwrap().to_string(),
                    STANDARD
                        .decode(f["content_base64"].as_str().unwrap())
                        .unwrap(),
                );
            }
            use sha2::{Digest, Sha256};
            let mut hash = Sha256::new();
            for (n, b) in files {
                hash.update((n.len() as u64).to_be_bytes());
                hash.update(n.as_bytes());
                hash.update((b.len() as u64).to_be_bytes());
                hash.update(b);
            }
            let digest = format!("{:x}", hash.finalize());
            state.record = json!({"id":format!("skill:global:_:{name}"),"revision":revision,"source_digest":digest,"enabled":true,"manifest":{"name":name,"version":"0.1.0","scope":payload["scope"],"permissions":[],"secret_refs":[],"spec":{"kind":"skill"}}});
            state.files = payload["files"].clone();
            if state.lost_import {
                state.lost_import = false;
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
            reply(json!({"capabilities":[state.record]}))
        }
        _ => {
            if path.starts_with("/v1/trace/runs/")
                && let Some(record) = state.trace_records.get(path.rsplit('/').next().unwrap())
            {
                return reply(record.clone());
            }
            if path.starts_with("/v1/trace/runs/run-") {
                let id = path.rsplit('/').next().unwrap();
                let turn = id.rsplit('-').next().unwrap().parse::<u64>().unwrap();
                let history = state.histories.get(&turn).cloned().unwrap();
                let learner = history["learner"] == true;
                let tokens = if learner { 10 } else { state.tokens };
                let mut invocations = if !state.record.is_null() && !learner {
                    let root = state
                        .files
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|f| f["path"] == "SKILL.md")
                        .unwrap();
                    let content = String::from_utf8(
                        STANDARD
                            .decode(root["content_base64"].as_str().unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                    json!([{"tool_name":"read_skill","arguments":{"id":state.record["id"]},"result":{"id":state.record["id"],"revision":state.record["revision"],"content":content},"outcome_status":"success"}])
                } else {
                    json!([])
                };
                if !learner
                    && !state.record.is_null()
                    && !state.omit_resource_read
                    && let Some(index) = state
                        .files
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|f| f["path"] == "resource-index.json")
                {
                    let content = String::from_utf8(
                        STANDARD
                            .decode(index["content_base64"].as_str().unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                    let value: Value = serde_json::from_str(&content).unwrap();
                    let selected: Vec<_> = value["resources"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .take(2)
                        .cloned()
                        .collect();
                    invocations.as_array_mut().unwrap().push(json!({"tool_name":"read_skill","arguments":{"id":state.record["id"],"path":"resource-index.json"},"result":{"id":state.record["id"],"revision":state.record["revision"],"path":"resource-index.json","content":content,"selected_resources":selected},"outcome_status":"success"}));
                }
                if state.wrong_read_revision
                    && !learner
                    && !invocations.as_array().unwrap().is_empty()
                {
                    invocations[0]["result"]["revision"] = json!(999);
                }
                if state.learner_uses_tool && learner {
                    invocations = json!([{"tool_name":"sandbox_run","arguments":{},"result":{},"outcome_status":"success"}]);
                }
                return reply(
                    json!({"run":{"id":id,"status":"succeeded","session_id":format!("session-{}",state.namespace)},"model_calls":[{"provider":"fixture","model":"fixture","usage_reported":!state.unknown_usage,"usage":{"input_tokens":tokens-1,"output_tokens":1,"total_tokens":tokens},"duration_ms":1}],"invocations":invocations}),
                );
            }
            if path.starts_with("/v1/capabilities/") {
                if path.ends_with("/permissions") {
                    return reply(json!([]));
                }
                if path.ends_with("/preparation") {
                    return reply(json!({"runtime_ready":true}));
                }
                if path.ends_with("/skill-instructions") && !state.record.is_null() {
                    let root = state
                        .files
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|f| f["path"] == "SKILL.md")
                        .unwrap();
                    let content = String::from_utf8(
                        STANDARD
                            .decode(root["content_base64"].as_str().unwrap())
                            .unwrap(),
                    )
                    .unwrap();
                    return reply(json!({"revision":state.record["revision"],"content":content}));
                }
                if path.ends_with("/skill-package") && !state.record.is_null() {
                    return reply(json!({"capability":state.record,"files":state.files}));
                }
                if path.ends_with("/enabled") && method == "PUT" {
                    if payload["expected_revision"] != state.record["revision"] {
                        return StatusCode::CONFLICT.into_response();
                    }
                    state.record["enabled"] = payload["enabled"].clone();
                    let revision = state.record["revision"].as_u64().unwrap() + 1;
                    state.record["revision"] = json!(revision);
                    return reply(state.record.clone());
                }
                if state.record.is_null() {
                    return StatusCode::NOT_FOUND.into_response();
                }
                return reply(state.record.clone());
            }
            StatusCode::NOT_FOUND.into_response()
        }
    }
}
