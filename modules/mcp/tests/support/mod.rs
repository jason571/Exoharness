use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use exoharness_contracts::capability::{
    CapabilityManifest, PermissionDecision, PermissionRecord, StoredCapability, ValidatedCapability,
};
use exoharness_secrets::SecretStore;
use serde_json::{Value, json};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::task::JoinHandle;
#[derive(Default)]
pub struct StateData {
    pub extra_result: Option<Value>,
    pub requests: Mutex<Vec<Value>>,
    pub headers: Mutex<Vec<HeaderMap>>,
    pub repeated_cursor: bool,
    pub progress_matches: bool,
    pub fail_call: bool,
    pub background: bool,
    pub background_nested: bool,
    pub background_conflicting: bool,
    pub background_failed: bool,
    pub background_never_complete: bool,
    pub background_poll_delay_ms: u64,
    pub release_background: AtomicBool,
    pub durable_workflow: bool,
    pub workflow_missing_preserve: bool,
    pub declared_audio: bool,
    pub extra_tools: usize,
    pub file_headers: Mutex<Vec<HeaderMap>>,
}
pub struct Server {
    pub url: String,
    pub state: Arc<StateData>,
    task: JoinHandle<()>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub async fn server(mut config: StateData) -> Server {
    config.requests = Mutex::new(Vec::new());
    let state = Arc::new(config);
    let app = Router::new()
        .route("/mcp", post(rpc).delete(|| async { StatusCode::OK }))
        .route("/files/song.mp3", get(audio))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    Server {
        url: format!("http://{address}/mcp"),
        state,
        task,
    }
}
async fn audio(State(state): State<Arc<StateData>>, headers: HeaderMap) -> Response {
    state.file_headers.lock().unwrap().push(headers);
    ([("content-type", "audio/mpeg")], "completed audio bytes").into_response()
}
async fn rpc(
    State(state): State<Arc<StateData>>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    state.headers.lock().unwrap().push(headers);
    state.requests.lock().unwrap().push(request.clone());
    let id = request["id"].clone();
    let result = match request["method"].as_str().unwrap_or("") {
        "initialize" => {
            json!({"protocolVersion":"2025-06-18","capabilities":{"tools":{}},"serverInfo":{"name":"fixture-server","version":"1"}})
        }
        "notifications/initialized" | "notifications/cancelled" => {
            return StatusCode::ACCEPTED.into_response();
        }
        "tools/list" => {
            if state.background {
                let schema = json!({"type":"object","properties":{"job_id":{"type":"string"}},"required":["job_id"]});
                return Json(json!({"jsonrpc":"2.0","id":id,"result":{"tools":[{"name":"echo","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}},{"name":"poll","inputSchema":schema},{"name":"cancel","inputSchema":schema}]}})).into_response();
            }
            let page = request["params"]["cursor"].as_str();
            if state.repeated_cursor {
                json!({"tools":[],"nextCursor":"same"})
            } else if page == Some("page2") {
                let mut tools = vec![
                    json!({"name":"other","description":"Other fixture tool","inputSchema":{"type":"object"}}),
                ];
                tools.extend((0..state.extra_tools).map(|index| json!({"name":format!("unused_{index}"),"description":"Unrelated optional operation","inputSchema":{"type":"object","properties":{"unused_payload":{"type":"string","description":"UNRELATED_FULL_SCHEMA"}}}})));
                json!({"tools":tools})
            } else {
                json!({"tools":[{"name":"echo","description":"Fixture echo","inputSchema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}],"nextCursor":"page2"})
            }
        }
        "tools/call" => {
            if let Some(result) = &state.extra_result {
                return Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response();
            }
            if state.declared_audio {
                let host = state.headers.lock().unwrap().last().unwrap()["host"]
                    .to_str()
                    .unwrap()
                    .to_owned();
                return Json(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"resource_link","uri":format!("http://{host}/files/song.mp3?key=file-only"),"name":"song.mp3","mimeType":"audio/mpeg"}],"isError":false}})).into_response();
            }
            if state.durable_workflow {
                let prepared = request["params"]["name"] == "echo";
                let directive = if prepared {
                    json!({"version":1,"status":"required","context_scope":"checkpoint","required_tool":"other","checkpoint":{"id":"saved-1"},"required_arguments":{"state_id":"saved-1"},"repair_context":{"exact":"retained input"}})
                } else {
                    json!({"version":1,"status":"complete","context_scope":"checkpoint"})
                };
                let mut meta = json!({"context_checkpoint":"persisted fixture state","workflow_continuation":directive});
                if state.workflow_missing_preserve {
                    meta["context_preserve"] = json!(["/missing"]);
                }
                return Json(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"OLD FULL RESULT".repeat(3000)}],"structuredContent":{"status":if prepared{"prepared"}else{"completed"},"custom_fact":"KEEP COMPLETE FACT","artifacts":if prepared{json!([])}else{json!([{"filename":"final.mp4","content_type":"video/mp4","download_url":"https://media.example/final.mp4"}])}},"_meta":{"exoharness":meta}}})).into_response();
            }
            if state.background {
                let name = request["params"]["name"].as_str().unwrap();
                if name == "poll" && state.background_poll_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(state.background_poll_delay_ms)).await;
                }
                let polls = state
                    .requests
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "poll")
                    .count();
                let status = if name == "cancel" {
                    "cancelled"
                } else if name == "poll"
                    && polls >= 5
                    && (!state.background_never_complete
                        || state.release_background.load(Ordering::SeqCst))
                {
                    if state.background_failed {
                        "failed"
                    } else {
                        "completed"
                    }
                } else {
                    "working"
                };
                let artifacts = if status == "completed" {
                    json!([{"filename":"final.mp4","content_type":"video/mp4","download_url":"https://media.example/final.mp4"}])
                } else {
                    json!([])
                };
                let mut result = json!({"content":[],"isError":status=="failed","structuredContent":{"status":status,"artifacts":artifacts},"_meta":{"exoharness":{"background_task":{"version":1,"id":"fixture-job","status":status,"poll":{"tool":"poll","arguments":{"job_id":"fixture-job"}},"cancel":{"tool":"cancel","arguments":{"job_id":"fixture-job"}},"poll_interval_ms":500,"progress":{"progress":polls,"total":5,"message":"fixture actual progress"}}}}});
                if state.background_nested || state.background_conflicting {
                    result["structuredContent"]["_meta"] = result["_meta"].clone();
                    if state.background_conflicting {
                        result["structuredContent"]["_meta"]["exoharness"]["background_task"]["id"] =
                            json!("different-job");
                    } else {
                        result.as_object_mut().unwrap().remove("_meta");
                    }
                }
                return Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response();
            }
            if state.fail_call {
                return (
                    StatusCode::OK,
                    [("content-type", "text/event-stream")],
                    "event: message\ndata: \n\n",
                )
                    .into_response();
            }
            if request["params"]["name"] == "long" {
                let token = if state.progress_matches {
                    request["params"]["_meta"]["progressToken"].clone()
                } else {
                    json!("wrong-token")
                };
                let frames=(1..=6).map(|progress|json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":token,"progress":progress,"total":6,"message":format!("Step {progress}")}})).chain(std::iter::once(json!({"jsonrpc":"2.0","id":id,"result":{"content":[{"type":"text","text":"finished"}]}}))).collect::<Vec<_>>();
                let (sender, receiver) = tokio::sync::mpsc::channel(16);
                tokio::spawn(async move {
                    for frame in frames {
                        tokio::time::sleep(Duration::from_millis(40)).await;
                        if sender
                            .send(Ok::<_, std::convert::Infallible>(format!(
                                "event: message\ndata: {frame}\n\n"
                            )))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                return Response::builder()
                    .header("content-type", "text/event-stream")
                    .body(Body::from_stream(
                        tokio_stream::wrappers::ReceiverStream::new(receiver),
                    ))
                    .unwrap();
            }
            json!({"content":[{"type":"text","text":"fixture result"}],"structuredContent":{"echo":request["params"]["arguments"],"custom_fact":"unaltered"},"_meta":{"provider_defined":"kept"}})
        }
        _ => return Json(
            json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"method not found"}}),
        )
        .into_response(),
    };
    let mut response = Json(json!({"jsonrpc":"2.0","id":id,"result":result})).into_response();
    response
        .headers_mut()
        .insert("mcp-session-id", "fixture-session".parse().unwrap());
    response
}
pub fn secrets() -> (tempfile::TempDir, SecretStore) {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let secrets = SecretStore::open(root.path().into()).unwrap();
    (root, secrets)
}
pub fn record(url: &str) -> (StoredCapability, Vec<PermissionRecord>) {
    let manifest:CapabilityManifest=serde_json::from_value(json!({"schema_version":1,"name":"fixture","description":"Test fixture","version":"1.0.0","scope":{"kind":"global"},"source":{"kind":"remote","url":url},"permissions":["network.egress"],"secret_refs":[],"spec":{"kind":"mcp_server","config":{"namespace":"fixture","transport":{"type":"streamable_http","url":url}}}})).unwrap();
    let validated = ValidatedCapability::new(manifest, None).unwrap();
    let manifest = validated.manifest().clone();
    let id = manifest.id();
    (
        StoredCapability {
            id: id.clone(),
            manifest,
            manifest_digest: validated.digest().into(),
            source_digest: None,
            revision: 1,
            enabled: true,
            created_at: 0,
            updated_at: 0,
        },
        vec![PermissionRecord {
            capability_id: id,
            permission: "network.egress".into(),
            decision: PermissionDecision::Allow,
            decided_by: Some("admin".into()),
            decided_at: Some(0),
        }],
    )
}
