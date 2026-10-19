use super::*;
pub(super) const ADMIN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const VIEWER: &str = "vvvvvvvvvvvvvvvvvvvvvvvvvvvvvvvv";
pub(super) fn management(f: &Fixture, surface: Surface) -> Router {
    let credentials = Arc::new(
        Credentials::from_json(
            &json!([
                {"id":"admin","role":"admin","token":ADMIN},
                {"id":"viewer","role":"chat","token":VIEWER},
                {"id":"user","role":"user","token":TOKEN}
            ])
            .to_string(),
        )
        .unwrap(),
    );
    let api = ApiState {
        execution: Arc::new(ExecutionService(f.store.clone())),
        credentials,
        surface,
        port: 9797,
        stop: f._stop.subscribe(),
    };
    let assistants =
        Arc::new(Assistants::initialize(&f._root.path().join("trace-assistants")).unwrap());
    exoharness_api::management_router(api, assistants)
}
pub(super) async fn get(router: &Router, path: &str, token: &str) -> (StatusCode, Value) {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("host", "127.0.0.1:9797")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}
#[tokio::test]
async fn failed_run_trace_is_visible_with_exact_arguments_result_usage_and_request() {
    let f=fixture(vec![
        tool("activate_tools",json!({"names":["sandbox_run"]})),
        tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"raise RuntimeError('retained failure diagnostic')"},"output_files":[]})),
        json!({"fixture_workspace_next":"sandbox_run","fixture_project_arguments":{"runtime":"python","entrypoint":"main.py","output_files":[]}}),
        json!({"fixture_workspace_next":"sandbox_run","fixture_project_arguments":{"runtime":"python","entrypoint":"main.py","output_files":[]}})
    ],true).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let router = management(&f, Surface::Management);
    let (status, sessions) = get(
        &router,
        &format!("/v1/trace/sessions?session_id={session}&limit=1"),
        ADMIN,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(sessions[0]["id"], session);
    // The inline-to-workspace transition no longer submits an unchanged rerun.
    assert_eq!(sessions[0]["model_call_count"], 3);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
    let (_, runs) = get(
        &router,
        &format!("/v1/trace/runs?session_id={session}"),
        ADMIN,
    )
    .await;
    assert_eq!(runs[0]["id"], run);
    assert_eq!(
        get(&router, "/v1/trace/sessions", VIEWER).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, detail) = get(
        &router,
        &format!("/v1/trace/runs/{run}?event_limit=1"),
        ADMIN,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["run"]["status"], "failed");
    assert_eq!(detail["invocations"].as_array().unwrap().len(), 2);
    assert_eq!(detail["events"].as_array().unwrap().len(), 1);
    assert!(
        detail["invocations"][1]["result"]["execution"]["stderr"]
            .as_str()
            .unwrap()
            .contains("retained failure diagnostic")
    );
    assert_eq!(
        detail["invocations"][1]["arguments"]["entrypoint"],
        "main.py"
    );
    assert_eq!(detail["invocations"][1]["outcome_status"], "failed");
    assert_eq!(
        detail["invocations"][1]["journal_outcome_status"],
        "failed_after_execution"
    );
    assert!(
        detail["model_calls"][0]["usage_reported"]
            .as_bool()
            .unwrap()
    );
    let call = &detail["model_calls"][1];
    let path = format!(
        "/v1/trace/runs/{run}/requests/{}/{}",
        call["logical_call_id"].as_str().unwrap(),
        call["attempt"]
    );
    let (status, evidence) = get(&router, &path, ADMIN).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(evidence["comparison_available"], true);
    assert!(
        !evidence["changes"]["retained"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let part = evidence["manifest"]["parts"][0]["sha256"].as_str().unwrap();
    assert_eq!(
        get(&router, &format!("{path}?part={part}"), ADMIN).await.0,
        StatusCode::OK
    );
    assert_eq!(
        get(&router, &format!("{path}?part=not-in-manifest"), ADMIN)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&router, &format!("/v1/trace/runs/{run}"), TOKEN)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&router, "/v1/trace/sessions?limit=0", ADMIN).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(&router, "/v1/trace/runs/absent", ADMIN).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        get(&router, "/v1/trace/sessions?session_id=absent", ADMIN)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        get(
            &management(&f, Surface::Portal),
            "/v1/trace/sessions",
            ADMIN
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let store = f.store.clone();
    let saved = session.clone();
    tokio::task::spawn_blocking(move || store.delete_conversation("user", &saved).unwrap())
        .await
        .unwrap();
    assert_eq!(
        get(&router, &format!("/v1/trace/runs/{run}"), ADMIN)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(get(&router, &path, ADMIN).await.0, StatusCode::OK);
    let (_, archived) = get(
        &router,
        &format!("/v1/trace/sessions?session_id={session}"),
        ADMIN,
    )
    .await;
    assert_eq!(archived[0]["archived"], 1);
    let store = f.store.clone();
    let saved = session.clone();
    assert!(
        tokio::task::spawn_blocking(move || store.conversation_detail("user", &saved, None, 8))
            .await
            .unwrap()
            .is_err()
    );
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn authorized_skill_paths_are_readonly_and_workspace_alias_publishes_output() {
    let source = "from pathlib import Path\nfor root in ['/skill','/work/skill','/workspace/skill']:\n p=Path(root)/'SKILL.md'\n assert p.read_text().startswith('---')\n try: p.write_text('tamper')\n except OSError: pass\n else: raise AssertionError('skill mount is writable')\nPath('/workspace/final.txt').write_text('中文最终产物')\nassert Path('/work/final.txt').read_text()=='中文最终产物'\n";
    let f=fixture(vec![tool("activate_tools",json!({"names":["sandbox_run"]})),tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":source},"skill_id":SKILL,"output_files":["final.txt"]})),answer()],true).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let router = management(&f, Surface::Management);
    let (_, detail) = get(&router, &format!("/v1/trace/runs/{run}"), ADMIN).await;
    assert_eq!(
        detail["invocations"][1]["result"]["execution"]["exit_code"],
        0
    );
    assert_eq!(
        detail["invocations"][1]["result"]["artifacts"][0]["filename"],
        "final.txt"
    );
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
