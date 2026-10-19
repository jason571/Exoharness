use super::*;
#[tokio::test]
async fn declared_background_task_waits_five_polls_without_model_repetition_and_delivers_video() {
    for (failed, nested) in [(false, false), (true, false), (false, true), (true, true)] {
        let remote = mcp_server::server(mcp_server::StateData {
            background: true,
            background_nested: nested,
            background_failed: failed,
            ..Default::default()
        })
        .await;
        let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
        let mut replies = vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"start once"})),
        ];
        if !failed {
            replies.push(answer());
        } else {
            replies.push(json!({"choices":[{"message":{"role":"assistant","content":"已有内容已保留，视频暂时无法完成。"},"finish_reason":"stop"}],"usage":{"prompt_tokens":50,"completion_tokens":10}}));
        }
        let f = fixture(replies, true).await;
        register_mcp(&f, &remote.url).await;
        let (session, run) = start(&f).await;
        assert_eq!(
            terminal(&f, &run).await,
            if failed {
                RunState::Failed
            } else {
                RunState::Succeeded
            }
        );
        let receipt = invocation(&f, &session, &run, 2).await.result.unwrap();
        assert_eq!(
            receipt["structuredContent"]["status"],
            if failed { "failed" } else { "completed" }
        );
        assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
        {
            let requests = remote.state.requests.lock().unwrap();
            assert_eq!(
                requests
                    .iter()
                    .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "echo")
                    .count(),
                1
            );
            assert_eq!(
                requests
                    .iter()
                    .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "poll")
                    .count(),
                5
            );
        }
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}
#[tokio::test]
async fn user_cancellation_cancels_the_declared_existing_job_once_without_resubmitting() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_nested: true,
        background_never_complete: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"start once"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start(&f).await;
    for _ in 0..100 {
        if remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["params"]["name"] == "poll")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{run}/cancel"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(terminal(&f, &run).await, RunState::Cancelled);
    for _ in 0..100 {
        if remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["params"]["name"] == "cancel")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    {
        let requests = remote.state.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "echo")
                .count(),
            1
        );
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "cancel")
                .count(),
            1
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn conflicting_background_declarations_stop_without_polling_or_resubmitting() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_conflicting: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"start once"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    {
        let requests = remote.state.requests.lock().unwrap();
        assert_eq!(
            requests
                .iter()
                .filter(|r| r["method"] == "tools/call")
                .count(),
            1
        );
    }
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn later_model_failure_keeps_completed_artifacts_in_history_and_terminal_event() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_nested: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let mut truncated = answer();
    truncated["choices"][0]["finish_reason"] = json!("length");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"start once"})),
            truncated,
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Failed);
    let store = f.store.clone();
    let saved = session.clone();
    let detail = tokio::task::spawn_blocking(move || {
        store.conversation_detail("user", &saved, None, 8).unwrap()
    })
    .await
    .unwrap();
    assert_eq!(
        detail["turns"][0]["response"]["artifacts"][0]["filename"],
        "final.mp4"
    );
    let store = f.store.clone();
    let saved = session.clone();
    let saved_run = run.clone();
    let events = tokio::task::spawn_blocking(move || {
        store.events("user", &saved, &saved_run, 0, 256).unwrap()
    })
    .await
    .unwrap();
    let events = serde_json::to_value(events).unwrap();
    assert!(events.as_array().unwrap().iter().any(
        |event| event["event"]["data"]["payload"]["type"] == "failed"
            && event["event"]["data"]["payload"]["artifacts"][0]["filename"] == "final.mp4"
    ));
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 3);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
