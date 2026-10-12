#[path = "../../model-runtime/tests/support/server.rs"]
#[allow(dead_code)]
mod server;
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use exoharness_api::{
    ApiState, ChatTasks, ExecutionService, Surface, auth::Credentials, chat_router_with_tasks,
    conversation_router,
};
use exoharness_configuration::{Assistants, Models};
use exoharness_model_runtime::{ModelRuntime, RuntimePolicy};
use exoharness_storage_postgres::{PostgresStore, RunState};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::sync::watch;
use tower::ServiceExt;
const USER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const OTHER: &str = "cccccccccccccccccccccccccccccccc";
struct Fixture {
    _root: tempfile::TempDir,
    router: Router,
    store: PostgresStore,
    _stop: watch::Sender<bool>,
    runtime: Arc<ModelRuntime>,
    models: Arc<Models>,
    tasks: Arc<ChatTasks>,
}
async fn fixture(upstream: &server::Server) -> Fixture {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let models = Arc::new(Models::initialize(&root.path().join("models")).unwrap());
    models.save_configuration(json!({"name":"fixture-api","api":"openai_compatible","base_url":upstream.url,"enabled":true}),
        json!({"name":"llm","provider":"fixture-api","model_id":"fixture-model","display_name":"Mock","enabled":true,"max_tokens":128,"thinking":"none"}),Some("private-test-credential"),None).unwrap();
    models.set_default("llm", None).unwrap();
    let assistants = Arc::new(Assistants::initialize(&root.path().join("assistants")).unwrap());
    assistants.create("test",json!({"display_name":"Test","avatar_text":"T","description":"Fixture","system_prompt":"You are the test assistant. Do not claim external actions without tool results.","skill_ids":[],"access":{"skills":{"mode":"none"},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"none"},"published":true}}),true,true).unwrap();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let namespace = format!(
        "pro_chat_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = tokio::task::spawn_blocking(move || {
        let s = PostgresStore::connect(
            &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated database"),
            &namespace,
        )
        .unwrap();
        s.migrate().unwrap();
        s
    })
    .await
    .unwrap();
    let credentials=Arc::new(Credentials::from_json(&json!([{ "id":"user","role":"user","token":USER},{"id":"other","role":"user","token":OTHER}]).to_string()).unwrap());
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
        conversation_router(api.clone(), assistants.clone()).merge(chat_router_with_tasks(
            api,
            models.clone(),
            assistants,
            runtime.clone(),
            tasks.clone(),
        ));
    Fixture {
        _root: root,
        router,
        store,
        _stop: stop,
        runtime,
        models,
        tasks,
    }
}
async fn raw(
    f: &Fixture,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> axum::response::Response {
    f.router
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn request(
    f: &Fixture,
    token: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = raw(f, token, method, path, body).await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
async fn start(f: &Fixture) -> (String, String) {
    let (status, session) = request(
        f,
        USER,
        "POST",
        "/v1/sessions",
        json!({"title":null,"actor":"spoofed"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{session}");
    let sid = session["id"].as_str().unwrap().to_owned();
    let (status,started)=request(f,USER,"POST",&format!("/v1/sessions/{sid}/turns"),json!({"model":null,"prompt":"hello","attachments":[],"capability_ids":[],"actor":"spoofed"})).await;
    assert_eq!(status, StatusCode::CREATED, "{started}");
    (sid, started["run_id"].as_str().unwrap().to_owned())
}
async fn terminal(f: &Fixture, run: &str) -> RunState {
    for _ in 0..100 {
        let store = f.store.clone();
        let run = run.to_owned();
        let record = tokio::task::spawn_blocking(move || store.owned_run("user", &run).unwrap())
            .await
            .unwrap();
        if !record.state.active() {
            return record.state;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("chat Run remained active");
}
#[tokio::test]
async fn real_upstream_deltas_are_persisted_and_visible_before_completion() {
    let first = json!({"id":"stream","choices":[{"index":0,"delta":{"role":"assistant","content":"开始回答"}}]});
    // Keep the upstream unfinished long enough for the SSE journal reader to
    // observe the first delta while the worker is still Running.
    let rest =
        json!({"id":"stream","choices":[{"index":0,"delta":{"content":"内容".repeat(1500)}}]});
    let end = json!({"id":"stream","choices":[{"index":0,"delta":{},"finish_reason":"stop"}]});
    let mut reply = server::Reply::json(Value::Null);
    reply.body = format!("data: {first}\n\ndata: {rest}\n\ndata: {end}\n\ndata: [DONE]\n\n");
    reply.content_type = "text/event-stream";
    reply.chunked = true;
    let upstream = server::Server::new(reply).await;
    let f = fixture(&upstream).await;
    let (_, run) = start(&f).await;
    let response = raw(
        &f,
        USER,
        "GET",
        &format!("/v1/runs/{run}/events"),
        Value::Null,
    )
    .await;
    let mut body = response.into_body();
    let mut all = String::new();
    tokio::time::timeout(Duration::from_secs(3), async {
        while let Some(frame) = body.frame().await {
            if let Ok(data) = frame.unwrap().into_data() {
                all.push_str(std::str::from_utf8(&data).unwrap());
                if all.contains("text_delta") {
                    break;
                }
            }
        }
    })
    .await
    .unwrap();
    assert!(all.contains("开始回答"));
    assert!(!all.contains("complete"));
    let store = f.store.clone();
    let rid = run.clone();
    assert_eq!(
        tokio::task::spawn_blocking(move || store.owned_run("user", &rid).unwrap().state)
            .await
            .unwrap(),
        RunState::Running
    );
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let replay = raw(
        &f,
        USER,
        "GET",
        &format!("/v1/runs/{run}/events"),
        Value::Null,
    )
    .await;
    let bytes = replay.into_body().collect().await.unwrap().to_bytes();
    let replay = std::str::from_utf8(&bytes).unwrap();
    assert_eq!(replay.matches("\"type\":\"text_delta\"").count(), 2);
    assert!(replay.contains("complete"));
    assert_eq!(upstream.count(), 1);
}
#[tokio::test]
async fn original_turn_api_calls_model_once_persists_answer_and_replays_scoped_sse() {
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let f = fixture(&upstream).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    assert_eq!(upstream.count(), 1);
    let (status, history) = request(
        &f,
        USER,
        "GET",
        &format!("/v1/sessions/{session}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        history["turns"][0]["answer"]
            .as_str()
            .is_some_and(|v| !v.is_empty())
    );
    for path in [
        format!("/v1/runs/{run}/events"),
        format!("/v1/runs/{run}/cancel"),
        format!("/v1/runs/{run}/resume"),
    ] {
        assert_eq!(
            request(
                &f,
                OTHER,
                if path.ends_with("events") {
                    "GET"
                } else {
                    "POST"
                },
                &path,
                json!({})
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    let response = raw(
        &f,
        USER,
        "GET",
        &format!("/v1/runs/{run}/events"),
        Value::Null,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = tokio::time::timeout(Duration::from_secs(2), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    let stream = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(stream.contains("event: chat"));
    assert!(stream.contains("assistant_start"));
    assert!(stream.contains("complete"));
    assert!(!stream.contains("private-test-credential"));
    let store = f.store.clone();
    let rid = run.clone();
    let version =
        tokio::task::spawn_blocking(move || store.owned_run("user", &rid).unwrap().version)
            .await
            .unwrap();
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/runs/{run}/events"))
                .header("authorization", format!("Bearer {USER}"))
                .header("last-event-id", version.to_string())
                .header("host", "127.0.0.1:9798")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let acknowledged = tokio::time::timeout(Duration::from_secs(2), response.into_body().collect())
        .await
        .unwrap()
        .unwrap()
        .to_bytes();
    assert!(acknowledged.is_empty());
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/runs/{run}/events"))
                .header("authorization", format!("Bearer {USER}"))
                .header("last-event-id", (version + 1).to_string())
                .header("host", "127.0.0.1:9798")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    f.runtime.close().await;
}
#[tokio::test]
async fn cancel_stops_provider_and_late_completion_cannot_reactivate_run() {
    let mut reply = server::Reply::json(server::answer());
    reply.delay = Duration::from_secs(5);
    let upstream = server::Server::new(reply).await;
    let f = fixture(&upstream).await;
    let (_, run) = start(&f).await;
    let cancel_path = format!("/v1/runs/{run}/cancel");
    let (first, second) = tokio::join!(
        request(&f, USER, "POST", &cancel_path, json!({})),
        request(&f, USER, "POST", &cancel_path, json!({}))
    );
    let (status, result) = first;
    assert_eq!(second.0, StatusCode::OK, "{}", second.1);
    assert_eq!(status, StatusCode::OK, "{result}");
    let (status, result) = request(
        &f,
        USER,
        "POST",
        &format!("/v1/runs/{run}/cancel"),
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(terminal(&f, &run).await, RunState::Cancelled);
    assert_eq!(
        request(
            &f,
            USER,
            "POST",
            &format!("/v1/runs/{run}/cancel"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &f,
            USER,
            "POST",
            &format!("/v1/runs/{run}/resume"),
            json!({})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    f.runtime.close().await;
}

#[tokio::test]
async fn core_task_shutdown_stops_active_model_and_rejects_new_turns() {
    let mut reply = server::Reply::json(server::answer());
    reply.delay = Duration::from_secs(5);
    let upstream = server::Server::new(reply).await;
    let f = fixture(&upstream).await;
    let (session, run) = start(&f).await;
    for _ in 0..100 {
        if upstream.count() > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(upstream.count(), 1);
    let ids = tokio::time::timeout(Duration::from_secs(2), f.tasks.close())
        .await
        .unwrap();
    assert_eq!(ids, vec![run.clone()]);
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let (status, history) = request(
        &f,
        USER,
        "GET",
        &format!("/v1/sessions/{session}"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let turns = history["turns"].as_array().unwrap().len();
    let (status, _) = request(
        &f,
        USER,
        "POST",
        &format!("/v1/sessions/{session}/turns"),
        json!({"prompt":"must not submit","model":null}),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    let (_, history) = request(
        &f,
        USER,
        "GET",
        &format!("/v1/sessions/{session}"),
        Value::Null,
    )
    .await;
    assert_eq!(history["turns"].as_array().unwrap().len(), turns);
    assert_eq!(upstream.count(), 1);
    f.runtime.close().await;
}

#[tokio::test]
async fn configured_model_output_limit_is_not_silently_clamped_by_chat() {
    let upstream = server::Server::new(server::Reply::json(server::answer())).await;
    let f = fixture(&upstream).await;
    f.models.save_configuration(json!({"name":"fixture-api","api":"openai_compatible","base_url":upstream.url,"enabled":true}),
        json!({"name":"llm","provider":"fixture-api","model_id":"fixture-model","display_name":"Mock","enabled":true,"max_tokens":24000,"thinking":"none"}),None,None).unwrap();
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    assert_eq!(
        upstream.state.received.lock().unwrap()[0].1["max_tokens"],
        24000
    );
    f.tasks.close().await;
    f.runtime.close().await;
}
#[tokio::test]
async fn provider_failure_stops_without_retry_and_unavailable_inputs_do_not_create_runs() {
    let mut reply = server::Reply::json(json!({"error":"rate limited"}));
    reply.status = StatusCode::TOO_MANY_REQUESTS;
    let upstream = server::Server::new(reply).await;
    let f = fixture(&upstream).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(upstream.count(), 1);
    assert_eq!(
        request(
            &f,
            USER,
            "POST",
            &format!("/v1/sessions/{session}/turns"),
            json!({"prompt":"file","attachments":[{"type":"file","name":"x"}]})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    let history = request(
        &f,
        USER,
        "GET",
        &format!("/v1/sessions/{session}"),
        Value::Null,
    )
    .await
    .1;
    assert_eq!(history["turns"].as_array().unwrap().len(), 1);
    assert_eq!(
        history["turns"][0]["response"]["public_notice"]["code"],
        "service_unavailable"
    );
    assert!(history["turns"][0]["response"].get("model_error").is_none());
    assert!(!history.to_string().contains("模型服务拒绝请求"));
    let store = f.store.clone();
    let (sid, rid) = (session.clone(), run.clone());
    let trace =
        tokio::task::spawn_blocking(move || store.events("user", &sid, &rid, 0, 256).unwrap())
            .await
            .unwrap();
    assert!(trace.iter().any(|e|matches!(&e.event,exoharness_storage_postgres::ExecutionEvent::Chat{payload} if payload["type"]=="execution_exception" && payload["model_error"]["http_status"]==429 && payload["severity"]=="error")));
    f.models.set_model_enabled("llm", false, None).unwrap();
    assert_eq!(
        request(
            &f,
            USER,
            "POST",
            &format!("/v1/sessions/{session}/turns"),
            json!({"prompt":"disabled"})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(upstream.count(), 1);
    f.runtime.close().await;
}
