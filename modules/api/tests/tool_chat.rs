//! Real API -> scripted provider -> Skill -> cached OCI -> signed artifact.
//! No paid model, host execution, remote deployment or image build.
#[path = "../../mcp/tests/support/mod.rs"]
#[allow(dead_code)]
mod mcp_server;
use axum::{
    Router,
    body::{Body, Bytes},
    extract::State,
    http::{Request, StatusCode},
    routing::post,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_api::{
    ApiState, ChatTasks, ExecutionService, Surface, auth::Credentials, chat_router_with_tools,
    conversation_router,
};
use exoharness_artifacts::Artifacts;
use exoharness_configuration::{Assistants, Models};
use exoharness_mcp::Mcp;
use exoharness_model_runtime::{ModelRuntime, RuntimePolicy};
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_secrets::SecretStore;
use exoharness_skills::Skills;
use exoharness_storage_postgres::{PostgresStore, RegistryAccess, RunState};
use exoharness_tools::ToolRuntime;
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;
use tower::ServiceExt;
const TOKEN: &str = "dddddddddddddddddddddddddddddddd";
const SKILL: &str = "skill:global:_:document-fixture";
#[path = "unit/chat_attachments.rs"]
mod chat_attachments;
#[path = "unit/coding.rs"]
mod coding;
#[path = "unit/durable_workflow.rs"]
mod durable_workflow;
#[path = "unit/input_correction.rs"]
mod input_correction;
#[path = "unit/mcp_admission.rs"]
mod mcp_admission;
#[path = "unit/mcp_background.rs"]
mod mcp_background;
#[path = "unit/mcp_delivery.rs"]
mod mcp_delivery;
#[path = "unit/model_tools.rs"]
mod model_tools;
#[path = "unit/publication_recovery.rs"]
mod publication_recovery;
#[path = "unit/run_admission.rs"]
mod run_admission;
#[path = "../../model-runtime/tests/support/speech.rs"]
#[allow(dead_code)]
mod speech_fixture;
#[path = "unit/tool_discovery.rs"]
mod tool_discovery;
#[path = "unit/tool_trace.rs"]
mod tool_trace;
#[path = "unit/tool_workspace.rs"]
mod tool_workspace;
#[derive(Clone)]
struct Upstream {
    replies: Arc<Mutex<VecDeque<Value>>>,
    requests: Arc<Mutex<Vec<Value>>>,
}
async fn reply(State(state): State<Upstream>, body: Bytes) -> axum::Json<Value> {
    let request: Value = serde_json::from_slice(&body).unwrap();
    state.requests.lock().unwrap().push(request.clone());
    let response = state
        .replies
        .lock()
        .unwrap()
        .pop_front()
        .expect("unexpected extra model request");
    if let Some(action) = response["fixture_workspace_next"].as_str() {
        let mut result = Value::Null;
        for message in request["messages"].as_array().unwrap() {
            if let Ok(state) = serde_json::from_str::<Value>(message["content"].as_str().unwrap()) {
                let saved = state
                    .pointer("/completed_tool_checkpoint/result")
                    .or_else(|| state.get("result"))
                    .unwrap_or(&state);
                if saved["workspace_id"].is_string() {
                    result = saved.clone();
                }
            }
        }
        let id = result["workspace_id"].as_str().unwrap();
        let revision = result["revision"].as_u64().unwrap();
        if let Some(mut args) = response.get("fixture_project_arguments").cloned() {
            args["workspace_id"] = json!(id);
            if action != "coding_search" {
                args["expected_revision"] = json!(revision);
            }
            return axum::Json(tool(action, args));
        }
        let args = match action {
            "workspace_edit" => {
                let files=response.get("fixture_workspace_files").cloned().unwrap_or_else(||json!({"index.html":format!("<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><title>文化旅游</title><h1>宣传网页</h1></html><!--{}-->","fixture-large-layout".repeat(300))}));
                json!({"workspace_id":id,"expected_revision":revision,"files":files})
            }
            "mixed_sandbox_run" | "saved_sandbox_run" => {
                let mut args = json!({"workspace_id":id,"expected_revision":revision,"runtime":"python","entrypoint":"build_deck.py","output_files":["report.pptx"]});
                if action == "mixed_sandbox_run" {
                    args["files"] = response["fixture_workspace_files"].clone();
                }
                return axum::Json(tool("sandbox_run", args));
            }
            "workspace_publish" => {
                json!({"workspace_id":id,"expected_revision":revision,"output_files":["index.html"]})
            }
            "workspace_read" => {
                json!({"workspace_id":id,"path":"index.html","offset":response["fixture_workspace_offset"].as_u64().unwrap(),"limit":80})
            }
            _ => panic!("invalid fixture action"),
        };
        return axum::Json(tool(action, args));
    }
    axum::Json(response)
}
fn tool(name: &str, args: Value) -> Value {
    json!({"id":"fixture-response","choices":[{"message":{"role":"assistant","content":"","tool_calls":[{"id":"provider-reused-id","type":"function","function":{"name":name,"arguments":args.to_string()}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":100,"completion_tokens":30}})
}
fn answer() -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":"已生成完整文件。"},"finish_reason":"stop"}],"usage":{"prompt_tokens":50,"completion_tokens":10}})
}
struct Fixture {
    _root: tempfile::TempDir,
    router: Router,
    store: PostgresStore,
    tasks: Arc<ChatTasks>,
    sandbox: Arc<Sandbox>,
    _stop: watch::Sender<bool>,
    upstream: Upstream,
    server: tokio::task::JoinHandle<()>,
    tools: Arc<ToolRuntime>,
    models: Arc<Models>,
}
async fn fixture(replies: Vec<Value>, grant: bool) -> Fixture {
    let upstream = Upstream {
        replies: Arc::new(Mutex::new(replies.into())),
        requests: Arc::new(Mutex::new(vec![])),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let router = Router::new()
        .route("/chat/completions", post(reply))
        .with_state(upstream.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let models = Arc::new(Models::initialize(&root.path().join("models")).unwrap());
    models.save_configuration(json!({"name":"test","api":"openai_compatible","base_url":url,"enabled":true}),json!({"name":"llm","provider":"test","model_id":"fixture-model","display_name":"Fixture","enabled":true,"max_tokens":4096,"thinking":"none"}),Some("test-only-private-credential"),None).unwrap();
    models.set_default("llm", None).unwrap();
    let assistants = Arc::new(Assistants::initialize(&root.path().join("assistants")).unwrap());
    assistants.create("test",json!({"display_name":"Test","avatar_text":"T","description":"Fixture","system_prompt":"Use registered tools for executable tasks.","skill_ids":[SKILL],"access":{"skills":{"mode":"selected","ids":[SKILL]},"mcp":{"mode":if grant{"all"}else{"none"}},"tools":{"mode":if grant{"all"}else{"none"}},"models":{"mode":"none"},"published":true}}),true,true).unwrap();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let ns = format!(
        "pro_toolchat_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let database =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("disposable PostgreSQL required");
    let (store, registry) = tokio::task::spawn_blocking(move || {
        let s = PostgresStore::connect(&database, &ns).unwrap();
        s.migrate().unwrap();
        // Existing workflow fixtures isolate Tool behavior. Run admission has
        // its own explicit-limit tests below, without changing production defaults.
        let mut config = s.scheduler_configuration().unwrap();
        config.max_active_runs_per_user = 16;
        config.max_global_agent_runs = 16;
        s.save_scheduler_configuration(config).unwrap();
        let r = PostgresStore::connect(&database, &format!("{ns}_registry")).unwrap();
        r.migrate_registry().unwrap();
        (s, Arc::new(Registry(r)))
    })
    .await
    .unwrap();
    let skills = Arc::new(Skills::open(root.path().join("skills"), registry.clone()).unwrap());
    let import_skills = skills.clone();
    let import_registry = registry.clone();
    tokio::task::spawn_blocking(move || {
    let imported=import_skills.import(&RegistryAccess::Administrator,&serde_json::from_value(json!({"files":[{"path":"SKILL.md","content_base64":STANDARD.encode("---\nname: document-fixture\ndescription: Generate a Chinese document in the sandbox.\n---\nUse sandbox_run, then publish only the final output file.\n")}]})).unwrap(),"admin").unwrap();
    import_registry
        .0
        .set_capability_enabled(
            &RegistryAccess::Administrator,
            &imported.id,
            imported.revision,
            true,
            "admin",
        )
        .unwrap();
    }).await.unwrap();
    let secrets = Arc::new(SecretStore::open(root.path().join("secrets")).unwrap());
    let mcp = Arc::new(Mcp::new(registry.clone(), secrets));
    let sandbox = Arc::new(
        Sandbox::open(
            root.path().join("runtime"),
            &std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE").expect("cached image required"),
        )
        .await
        .unwrap(),
    );
    let artifacts = Arc::new(
        Artifacts::open(
            root.path().join("artifacts"),
            store.clone(),
            "http://127.0.0.1:9790",
        )
        .unwrap(),
    );
    let tools = Arc::new(ToolRuntime {
        execution: store.clone(),
        registry,
        skills,
        mcp,
        sandbox: Some(sandbox.clone()),
        artifacts,
        model_runtime: Some(Arc::new(
            ModelRuntime::new(models.clone(), RuntimePolicy::default()).unwrap(),
        )),
    });
    let credentials = Arc::new(
        Credentials::from_json(&json!([{"id":"user","role":"user","token":TOKEN}]).to_string())
            .unwrap(),
    );
    let (stop, rx) = watch::channel(false);
    let api = ApiState {
        execution: Arc::new(ExecutionService(store.clone())),
        credentials,
        surface: Surface::Portal,
        port: 9798,
        stop: rx,
    };
    let runtime = Arc::new(ModelRuntime::new(models.clone(), RuntimePolicy::default()).unwrap());
    let tasks = Arc::new(ChatTasks::default());
    let router =
        conversation_router(api.clone(), assistants.clone()).merge(chat_router_with_tools(
            api,
            models.clone(),
            assistants,
            runtime,
            tasks.clone(),
            Some(tools.clone()),
        ));
    Fixture {
        _root: root,
        router,
        store,
        tasks,
        sandbox,
        _stop: stop,
        upstream,
        server,
        tools,
        models,
    }
}
async fn request(f: &Fixture, path: &str, body: Value) -> Value {
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
async fn start(f: &Fixture) -> (String, String) {
    start_prompt(f, "生成一份中文PPT").await
}
async fn start_prompt(f: &Fixture, prompt: &str) -> (String, String) {
    let session = request(f, "/v1/sessions", json!({"title":null})).await["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let run = request(
        f,
        &format!("/v1/sessions/{session}/turns"),
        json!({"model":null,"prompt":prompt,"attachments":[],"capability_ids":[]}),
    )
    .await["run_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (session, run)
}
async fn terminal(f: &Fixture, run: &str) -> RunState {
    for _ in 0..400 {
        let s = f.store.clone();
        let id = run.to_owned();
        let state = tokio::task::spawn_blocking(move || s.owned_run("user", &id).unwrap().state)
            .await
            .unwrap();
        if !state.active() {
            return state;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("tool chat did not terminate");
}
#[tokio::test]
async fn skill_to_actual_oci_pptx_and_final_artifact_uses_native_paired_calls_and_usage() {
    let f=fixture(vec![tool("search_capabilities",json!({"query":""})),tool("read_skill",json!({"id":SKILL})),tool("activate_tools",json!({"names":["sandbox_run"]})),tool("sandbox_run",json!({"runtime":"javascript","entrypoint":"main.js","files":{"main.js":"const pptxgen=require('pptxgenjs'); const p=new pptxgen(); p.addSlide().addText('中文标题：测试交付',{x:1,y:1,w:8,h:1,fontFace:'Noto Sans CJK SC',fontSize:28}); p.writeFile({fileName:'final.pptx'});"},"output_files":["final.pptx"]})),answer()],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let store = f.store.clone();
    let sid = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &sid, None, 8).unwrap()
    })
    .await
    .unwrap();
    let response = &detail["turns"][0]["response"];
    assert_eq!(response["usage"]["total_tokens"], 580);
    assert_eq!(
        response["artifacts"][0]["filename"], "final.pptx",
        "{detail}"
    );
    assert!(
        response["artifacts"][0]["download_url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:9790/artifacts/")
    );
    let url = url::Url::parse(response["artifacts"][0]["download_url"].as_str().unwrap()).unwrap();
    let delivered = exoharness_api::media_router(f.tools.artifacts.clone(), 9790)
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                .header("host", "127.0.0.1:9790")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delivered.status(), StatusCode::OK);
    assert!(
        delivered.headers()["content-type"]
            .to_str()
            .unwrap()
            .contains("presentationml")
    );
    let bytes = delivered.into_body().collect().await.unwrap().to_bytes();
    assert!(bytes.len() > 1000 && bytes.starts_with(b"PK"));
    let record = invocation(&f, &session, &run, 4).await;
    assert_eq!(record.attempts, 1);
    assert_eq!(record.result.unwrap()["execution"]["exit_code"], 0);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 5);
        assert!(
            requests[0]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["function"]["name"] != "sandbox_run")
        );
        assert!(
            requests[3]["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["function"]["name"] == "sandbox_run")
        );
        assert!(requests.iter().all(|r| r["reasoning_effort"] == "none"));
        for request in requests.iter().skip(1) {
            let messages = request["messages"].as_array().unwrap();
            for (i, msg) in messages
                .iter()
                .enumerate()
                .filter(|(_, m)| m.get("tool_calls").is_some())
            {
                let id = msg["tool_calls"][0]["id"].as_str().unwrap();
                assert_eq!(messages[i + 1]["tool_call_id"], id);
            }
        }
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn unauthorized_execution_is_not_loaded_and_stops_without_a_container_job() {
    let f = fixture(
        vec![tool("activate_tools", json!({"names":["sandbox_run"]}))],
        false,
    )
    .await;
    let (_session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 1);
    assert_eq!(
        std::fs::read_dir(f._root.path().join("runtime/jobs"))
            .unwrap()
            .count(),
        0
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

async fn register_mcp(f: &Fixture, url: &str) {
    use exoharness_contracts::capability::PermissionDecision;
    let tools = f.tools.clone();
    let url = url.to_owned();
    let id=tokio::task::spawn_blocking(move||{
    let record=tools.mcp.import(&RegistryAccess::Administrator,&serde_json::from_value(json!({"content":json!({"mcpServers":{"fixture":{"type":"streamableHttp","url":url}}}).to_string()})).unwrap(),"admin").unwrap().remove(0);
    for permission in &record.manifest.permissions {
        tools
            .registry
            .0
            .decide_capability_permission(
                &RegistryAccess::Administrator,
                &record.id,
                record.revision,
                permission,
                PermissionDecision::Allow,
                "admin",
            )
            .unwrap();
    }
    tools
        .registry
        .0
        .set_capability_enabled(
            &RegistryAccess::Administrator,
            &record.id,
            record.revision,
            true,
            "admin",
        )
        .unwrap();
    record.id
    }).await.unwrap();
    f.tools
        .mcp
        .validate(RegistryAccess::Administrator, id, "admin".into())
        .await
        .unwrap();
}
#[tokio::test]
async fn registered_mcp_calls_keep_native_arguments_and_raw_provider_facts() {
    let remote = mcp_server::server(mcp_server::StateData::default()).await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"中文输入"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let receipt = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(receipt["structuredContent"]["echo"]["text"], "中文输入");
    assert_eq!(receipt["structuredContent"]["custom_fact"], "unaltered");
    assert_eq!(receipt["_meta"]["provider_defined"], "kept");
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        1
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn mcp_unknown_outcome_is_never_retried_by_another_model_request() {
    let remote = mcp_server::server(mcp_server::StateData {
        fail_call: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"once"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let receipt = invocation(&f, &session, &run, 2).await;
    assert_eq!(
        receipt.state,
        exoharness_contracts::invocation::InvocationState::OutcomeUnknown
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .count(),
        1
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
async fn invocation(
    f: &Fixture,
    session: &str,
    run: &str,
    sequence: u32,
) -> exoharness_contracts::execution::InvocationRecord {
    let store = f.store.clone();
    let session = session.to_owned();
    let run = run.to_owned();
    tokio::task::spawn_blocking(move || {
        store
            .invocation(
                "user",
                &session,
                &run,
                &format!("{run}:model-{sequence}-tool-0"),
            )
            .unwrap()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn duplicate_skill_read_does_not_load_its_body_again() {
    let f = fixture(
        vec![
            tool("read_skill", json!({"id":SKILL})),
            tool("read_skill", json!({"id":SKILL})),
            answer(),
        ],
        false,
    )
    .await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let repeated = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(repeated["already_loaded"], true);
    assert!(repeated.get("content").is_none());
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(
            requests[2]["messages"]
                .to_string()
                .matches("Generate a Chinese document in the sandbox.")
                .count(),
            1
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn loading_an_executable_tool_cannot_fake_completion() {
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":["sandbox_run"]})),
            answer(),
        ],
        true,
    )
    .await;
    let (_session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(
        std::fs::read_dir(f._root.path().join("runtime/jobs"))
            .unwrap()
            .count(),
        0
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn cancel_during_sandbox_execution_stops_model_followups_and_drains_container() {
    let f=fixture(vec![tool("activate_tools",json!({"names":["sandbox_run"]})),tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"import time\ntime.sleep(20)\n"},"output_files":[]}))],true).await;
    let (_session, run) = start(&f).await;
    for _ in 0..100 {
        if f.upstream.requests.lock().unwrap().len() == 2
            && std::fs::read_dir(f._root.path().join("runtime/jobs"))
                .unwrap()
                .count()
                > 0
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
    let stopped = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{run}/cancel"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stopped.status(), StatusCode::OK);
    assert_eq!(terminal(&f, &run).await, RunState::Cancelled);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    f.server.abort();
}
