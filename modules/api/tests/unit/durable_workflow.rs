use super::*;

#[tokio::test]
async fn durable_mcp_stages_shrink_context_and_schemas_without_mutating_exact_journal() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let source = "EXACT AUTHORED PLAN".repeat(1800);
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[prepare]})),
            tool(&prepare, json!({"text":source})),
            tool(&finish, json!({"state_id":"saved-1"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    let receipt = invocation(&f, &session, &run, 2).await;
    assert_eq!(receipt.intent.arguments["text"], source);
    assert!(
        receipt.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("OLD FULL RESULT")
    );
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        let stage = requests[2].to_string();
        assert!(!stage.contains("EXACT AUTHORED PLAN"));
        assert!(!stage.contains("OLD FULL RESULT"));
        assert!(stage.contains("retained input"));
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&finish.as_str()));
        assert!(!names.contains(&prepare.as_str()));
        let final_request = requests[3].to_string();
        assert!(final_request.contains("KEEP COMPLETE FACT"));
        assert!(final_request.contains("final.mp4"));
        assert!(!final_request.contains("OLD FULL RESULT"));
        let names: Vec<_> = requests[3]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(!names.contains(&finish.as_str()));
        assert!(!names.contains(&prepare.as_str()));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn missing_declared_checkpoint_value_keeps_exact_result_and_normal_tools() {
    let remote = mcp_server::server(mcp_server::StateData {
        durable_workflow: true,
        workflow_missing_preserve: true,
        ..Default::default()
    })
    .await;
    let prepare = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let finish = exoharness_tools::public_name("mcp_server:global:_:fixture", "other");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[prepare,finish]})),
            tool(&prepare, json!({"text":"exact input"})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start(&f).await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert!(requests[2].to_string().contains("OLD FULL RESULT"));
        let names: Vec<_> = requests[2]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["function"]["name"].as_str().unwrap())
            .collect();
        assert!(names.contains(&prepare.as_str()));
        assert!(names.contains(&finish.as_str()));
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
