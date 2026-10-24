use super::*;

fn selection_schema() -> Value {
    // A generic cross-field contract, independent of any application or MCP.
    json!({"type":"object","properties":{"text":{"type":"string"},
    "selection":{"$ref":"#/$defs/Selection"}},"required":["text"],
    "$defs":{"Selection":{"type":"object","additionalProperties":false,
        "properties":{"mode":{"enum":["direct","catalog"]},
            "catalog_id":{"type":["string","null"]},
            "document_id":{"type":["string","null"]}},
        "required":["mode"],"allOf":[{
            "if":{"properties":{"mode":{"const":"direct"}},"required":["mode"]},
            "then":{"properties":{"catalog_id":{"type":"null"},"document_id":{"type":"null"}}},
            "else":{"required":["catalog_id","document_id"],
                "properties":{"catalog_id":{"type":"string","minLength":1},
                    "document_id":{"type":"string","minLength":1}}}
        }]}}})
}

#[tokio::test]
async fn followup_corrects_conditional_input_before_one_remote_call_and_preserves_prior_turn() {
    let remote = mcp_server::server(mcp_server::StateData {
        echo_input_schema: Some(selection_schema()),
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(vec![
        tool("activate_tools",json!({"names":[name]})),
        tool(&name,json!({"text":"original persisted result"})),
        json!({"choices":[{"message":{"role":"assistant","content":"original persisted result"},"finish_reason":"stop"}],"usage":{"prompt_tokens":50,"completion_tokens":10}}),
        tool("activate_tools",json!({"names":[name]})),
        tool(&name,json!({"text":"continue exact input","selection":{"mode":"catalog"}})),
        tool(&name,json!({"text":"continue exact input","selection":{"mode":"catalog","catalog_id":"selected","document_id":null}})),
        tool(&name,json!({"text":"continue exact input","selection":{"mode":"catalog","catalog_id":"selected","document_id":"verified"}})),
        answer(),
    ],true).await;
    register_mcp(&f, &remote.url).await;
    let (session, first) = start_prompt(&f, "Produce the original result").await;
    assert_eq!(terminal(&f, &first).await, RunState::Succeeded);
    let response = request(
        &f,
        &format!("/v1/sessions/{session}/turns"),
        json!({"prompt":"Continue"}),
    )
    .await;
    let next = response["run_id"].as_str().unwrap();
    assert_eq!(terminal(&f, next).await, RunState::Succeeded);
    {
        let calls = remote.state.requests.lock().unwrap();
        let calls: Vec<_> = calls
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .collect();
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0]["params"]["arguments"]["text"],
            "original persisted result"
        );
        assert_eq!(
            calls[1]["params"]["arguments"]["selection"]["document_id"],
            "verified"
        );
    }
    {
        let requests = f.upstream.requests.lock().unwrap();
        assert_eq!(requests.len(), 8);
        for index in [5, 6] {
            let messages = requests[index]["messages"].to_string();
            assert!(messages.contains("_host_input_rejection"));
            assert!(messages.contains("continue exact input"));
            assert!(messages.contains("original persisted result"));
            assert!(!messages.contains("_host_authorization_denied"));
        }
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn opaque_checkpoint_id_is_corrected_before_any_remote_submission() {
    let remote = mcp_server::server(mcp_server::StateData {
        echo_input_schema: Some(json!({"type":"object","required":["text"],
            "properties":{"text":{"type":"string","pattern":"^saved_[0-9a-f]{64}$"}}})),
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let saved = format!("saved_{}", "a".repeat(64));
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"invented-title"})),
            tool(&name, json!({"text":saved})),
            answer(),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, run) = start_prompt(&f, "Use the saved checkpoint").await;
    assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
    {
        let calls = remote.state.requests.lock().unwrap();
        let calls: Vec<_> = calls
            .iter()
            .filter(|r| r["method"] == "tools/call")
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["params"]["arguments"]["text"], saved);
        let requests = f.upstream.requests.lock().unwrap();
        assert!(
            requests[2]["messages"]
                .to_string()
                .contains("_host_input_rejection")
        );
        assert!(
            !requests[2]["messages"]
                .to_string()
                .contains("_host_authorization_denied")
        );
    }
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
