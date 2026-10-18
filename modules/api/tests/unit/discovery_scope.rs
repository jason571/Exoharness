use super::*;

const RAW_PROMPT: &str = "为ExoHarness生成宣传网站页面";

fn production_mistake() -> Value {
    json!({"kind":"capability","capability_id":"skill:global:_:web-site-builder","query":"web site builder landing page HTML"})
}

async fn saved_response(f: &Fixture, session: &str) -> Value {
    let store = f.store.clone();
    let session = session.to_owned();
    tokio::task::spawn_blocking(move || {
        store
            .conversation_detail("user", &session, None, 8)
            .unwrap()["turns"][0]["response"]
            .clone()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn discovery_scope_matrix_corrects_before_real_oci_html_delivery() {
    for bad in [
        production_mistake(),
        json!({"kind":"skill","capability_id":SKILL,"query":"web"}),
        json!({"kind":"tool","capability_id":SKILL,"query":""}),
        json!({"kind":"tool","capability_id":"invented-service","query":""}),
        json!({"kind":"tool","capability_id":"mcp:inaccessible","query":""}),
        json!({"kind":"capability","capability_id":"tool:native:sandbox","query":""}),
    ] {
        let f = fixture(
            vec![
                tool("search_capabilities", bad.clone()),
                tool("search_capabilities", json!({"kind":"capability","query":""})),
                tool("search_capabilities", json!({"kind":"tool","capability_id":"tool:native:sandbox","query":""})),
                tool("activate_tools", json!({"names":["sandbox_run"]})),
                tool("sandbox_run", json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"from pathlib import Path\nPath('index.html').write_text('<!doctype html><html><meta charset=\"utf-8\"><title>ExoHarness</title><h1>ExoHarness</h1></html>')\n"},"output_files":["index.html"]})),
                answer(),
            ],
            true,
        )
        .await;
        let (session, run) = start_prompt(&f, RAW_PROMPT).await;
        assert_eq!(terminal(&f, &run).await, RunState::Succeeded, "{bad}");
        let rejected = invocation(&f, &session, &run, 1).await;
        assert_eq!(rejected.intent.arguments, bad);
        let receipt = rejected.result.unwrap();
        assert_eq!(receipt["_host_input_rejection"], 1);
        assert_eq!(receipt["code_executed"], false);
        assert_eq!(receipt["_host_execution"], false);
        assert!(receipt.get("_host_authorization_denied").is_none());
        let delivered = invocation(&f, &session, &run, 5).await.result.unwrap();
        assert_eq!(delivered["execution"]["exit_code"], 0);
        let response = saved_response(&f, &session).await;
        assert!(response.get("public_notice").is_none());
        assert_eq!(response["artifacts"].as_array().unwrap().len(), 1);
        let url =
            url::Url::parse(response["artifacts"][0]["download_url"].as_str().unwrap()).unwrap();
        let file = exoharness_api::media_router(f.tools.artifacts.clone(), 9790)
            .oneshot(
                Request::builder()
                    .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                    .header("host", "127.0.0.1:9790")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(file.status(), StatusCode::OK);
        let bytes = file.into_body().collect().await.unwrap().to_bytes();
        assert!(
            std::str::from_utf8(&bytes)
                .unwrap()
                .contains("<h1>ExoHarness</h1>")
        );
        {
            let requests = f.upstream.requests.lock().unwrap();
            assert_eq!(requests.len(), 6);
            let system = requests[0]["messages"][0]["content"].as_str().unwrap();
            assert!(
                system.contains("Persona facts describe the assistant's own organization only.")
            );
            assert!(system.contains("ask one concise clarification before authoring"));
        }
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}

#[tokio::test]
async fn unchanged_discovery_mistake_is_not_dispatched_twice() {
    let bad = production_mistake();
    let f = fixture(
        vec![
            tool("search_capabilities", bad.clone()),
            tool("search_capabilities", bad),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start_prompt(&f, RAW_PROMPT).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    let response = saved_response(&f, &session).await;
    assert_ne!(
        response["public_notice"]["code"],
        "capability_permission_required"
    );
    assert!(response["artifacts"].as_array().unwrap().is_empty());
    let store = f.store.clone();
    let rid = run.clone();
    let trace = tokio::task::spawn_blocking(move || store.trace_run(&rid, 0, 256).unwrap())
        .await
        .unwrap();
    // Only the first model proposal reaches dispatch; repetition stops earlier.
    assert_eq!(trace["invocations"].as_array().unwrap().len(), 1);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn changing_discovery_scopes_cannot_exceed_two_input_corrections() {
    let f = fixture(
        vec![
            tool("search_capabilities", production_mistake()),
            tool(
                "search_capabilities",
                json!({"kind":"tool","capability_id":"invented-a","query":""}),
            ),
            tool(
                "search_capabilities",
                json!({"kind":"tool","capability_id":"invented-b","query":""}),
            ),
            answer(),
        ],
        true,
    )
    .await;
    let (session, run) = start_prompt(&f, RAW_PROMPT).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
    let response = saved_response(&f, &session).await;
    assert_ne!(
        response["public_notice"]["code"],
        "capability_permission_required"
    );
    assert!(response["artifacts"].as_array().unwrap().is_empty());
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn corrected_discovery_never_grants_denied_native_execution() {
    let f = fixture(
        vec![
            tool("search_capabilities", production_mistake()),
            tool(
                "search_capabilities",
                json!({"kind":"capability","query":""}),
            ),
            tool("activate_tools", json!({"names":["sandbox_run"]})),
            answer(),
        ],
        false,
    )
    .await;
    let (session, run) = start_prompt(&f, RAW_PROMPT).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let denied = invocation(&f, &session, &run, 3).await.result.unwrap();
    assert_eq!(denied["_host_authorization_denied"], 1);
    assert_eq!(denied["_host_execution"], false);
    let response = saved_response(&f, &session).await;
    assert_eq!(
        response["public_notice"]["code"],
        "capability_permission_required"
    );
    assert!(response["artifacts"].as_array().unwrap().is_empty());
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn missing_or_non_skill_read_identity_is_correctable_before_execution() {
    for id in ["skill:global:_:invented", "mcp_server:global:_:fixture"] {
        let remote = mcp_server::server(mcp_server::StateData::default()).await;
        let f = fixture(
            vec![
                tool("read_skill", json!({"id":id})),
                tool("read_skill", json!({"id":SKILL})),
                answer(),
            ],
            true,
        )
        .await;
        register_mcp(&f, &remote.url).await;
        let (session, run) = start_prompt(&f, "Read the applicable authorized instructions before asking for the missing product brief.").await;
        assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
        let rejected = invocation(&f, &session, &run, 1).await.result.unwrap();
        assert_eq!(rejected["_host_input_rejection"], 1);
        assert_eq!(rejected["_host_execution"], false);
        assert!(rejected.get("_host_authorization_denied").is_none());
        let accepted = invocation(&f, &session, &run, 2).await.result.unwrap();
        assert!(
            accepted["content"]
                .as_str()
                .unwrap()
                .contains("document-fixture")
        );
        assert_eq!(
            remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|r| r["method"] == "tools/call")
                .count(),
            0
        );
        assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}

#[tokio::test]
async fn revoking_a_frozen_skill_still_stops_reading_without_input_correction() {
    let f = fixture(
        vec![tool("read_skill", json!({"id":SKILL})), answer()],
        true,
    )
    .await;
    f.upstream.paused.store(true, Ordering::SeqCst);
    let (session, run) = start_prompt(&f, "Read the authorized Skill instructions.").await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.upstream.requests.lock().unwrap().is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let registry = f.tools.registry.clone();
    tokio::task::spawn_blocking(move || {
        let record = registry
            .0
            .capability(&RegistryAccess::Administrator, SKILL)
            .unwrap();
        registry
            .0
            .set_capability_enabled(
                &RegistryAccess::Administrator,
                SKILL,
                record.revision,
                false,
                "admin",
            )
            .unwrap();
    })
    .await
    .unwrap();
    f.upstream.paused.store(false, Ordering::SeqCst);
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let receipt = invocation(&f, &session, &run, 1).await.result.unwrap();
    assert_eq!(receipt["_host_authorization_denied"], 1);
    assert_eq!(receipt["_host_execution"], false);
    assert!(receipt.get("content").is_none());
    assert_eq!(
        saved_response(&f, &session).await["public_notice"]["code"],
        "capability_permission_required"
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 1);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
