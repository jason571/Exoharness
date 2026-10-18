use super::*;

#[tokio::test]
async fn finalization_model_failure_or_tool_proposal_keeps_safe_response_without_execution() {
    for bad in [
        json!({"error":{"message":"private error"}}),
        tool("must-not-execute", json!({})),
    ] {
        let remote = mcp_server::server(mcp_server::StateData {
            extra_result: Some(
                json!({"isError":true,"content":[{"type":"text","text":"input unavailable"}]}),
            ),
            ..Default::default()
        })
        .await;
        let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
        let f = fixture(
            vec![
                tool("activate_tools", json!({"names":[name]})),
                tool(&name, json!({"text":"saved-input"})),
                bad,
            ],
            true,
        )
        .await;
        register_mcp(&f, &remote.url).await;
        let (session, run) = start(&f).await;
        assert_eq!(terminal(&f, &run).await, RunState::Failed);
        let store = f.store.clone();
        let sid = session.clone();
        let detail = tokio::task::spawn_blocking(move || {
            store.conversation_detail("user", &sid, None, 8).unwrap()
        })
        .await
        .unwrap();
        let response = &detail["turns"][0]["response"];
        assert_eq!(response["partial_result"], true);
        assert_eq!(response["model"]["name"], "llm");
        assert!(!response["answer"].as_str().unwrap().is_empty());
        assert!(
            !response["answer"]
                .as_str()
                .unwrap()
                .contains("private error")
        );
        assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
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
}

#[tokio::test]
async fn unknown_submission_blocks_continue_before_any_model_or_second_business_call() {
    let remote = mcp_server::server(mcp_server::StateData {
        fail_call: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"submit-once"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    terminal(&f, &run).await;
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/sessions/{session}/turns"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"prompt":"继续生成"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let value: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(value["error"]["code"], "result_pending_verification");
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

fn text_answer(text: &str) -> Value {
    json!({"choices":[{"message":{"role":"assistant","content":text},"finish_reason":"stop"}],"usage":{"prompt_tokens":50,"completion_tokens":10}})
}

#[tokio::test]
async fn failed_mcp_delivers_partial_answer_and_continue_reuses_exact_saved_inputs() {
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let saved=json!({"saved_plan":"lesson-original","audio":["audio-1","audio-2","audio-3","audio-4","audio-5","audio-6"]}).to_string();
    let remote=mcp_server::server(mcp_server::StateData {
        extra_results:Mutex::new(vec![
            json!({"isError":true,"content":[{"type":"text","text":"Temporary input read failure"}],"structuredContent":{"status":"input_unavailable","submitted":false,"phase":"metadata","retained_arguments":{"text":saved}}}),
            json!({"isError":false,"content":[{"type":"text","text":"Existing inputs completed"}],"structuredContent":{"status":"completed","saved_plan":"lesson-original"}})
        ].into()),..Default::default()
    }).await;
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":saved})),
            text_answer("39÷3=13，13×24=312。已有配音已保留，动画暂时不可用。"),
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":saved})),
            text_answer("已复用原来的配音完成讲解。"),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start_prompt(&f, "琪琪计算24与27的积相差39，用四年级方法讲解").await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let store = f.store.clone();
    let sid = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &sid, None, 8).unwrap()
    })
    .await
    .unwrap();
    assert_eq!(detail["turns"][0]["response"]["partial_result"], true);
    assert!(
        detail["turns"][0]["answer"]
            .as_str()
            .unwrap()
            .contains("312")
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 3);
        assert!(
            requests[2]
                .get("tools")
                .is_none_or(|v| v.as_array().is_some_and(Vec::is_empty))
        );
        assert!(requests[2].to_string().contains("lesson-original"));
    }
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/sessions/{session}/turns"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({"prompt":"继续生成"}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let next = body["run_id"].as_str().unwrap();
    assert_eq!(terminal(&f, next).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        let continuation = requests[3].to_string();
        assert!(continuation.contains("琪琪"));
        assert!(continuation.contains("lesson-original"));
        assert!(continuation.contains("audio-6"));
        assert_eq!(requests.len(), 6);
    }
    let calls: Vec<_> = remote
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["method"] == "tools/call")
        .cloned()
        .collect();
    assert_eq!(
        calls.len(),
        2,
        "One failed preparation and one explicit continuation, without automatic resubmission"
    );
    assert_eq!(
        calls[0]["params"]["arguments"],
        calls[1]["params"]["arguments"]
    );
    assert_eq!(
        terminal(&f, &run).await,
        RunState::Failed,
        "Original terminal facts stay unchanged"
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
