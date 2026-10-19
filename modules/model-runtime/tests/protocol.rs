#[allow(dead_code)]
mod support;
use exoharness_model_runtime::{
    CancellationToken, ErrorKind, Finish, Message, RuntimePolicy, Tool, ToolCall, ToolChoice,
};
use serde_json::json;
use support::{
    Fixture, request,
    server::{Reply, answer},
};
fn tool() -> Tool {
    Tool {
        name: "calculate".into(),
        description: "Calculate using native arguments.".into(),
        input_schema: json!({"type":"object","properties":{"number":{"type":"integer"}},"required":["number"]}),
    }
}
fn proposal() -> ToolCall {
    ToolCall {
        id: "call-a".into(),
        name: "calculate".into(),
        arguments: json!({"number":100}),
    }
}
#[tokio::test]
async fn returned_ids_cannot_reuse_historical_invocations_or_duplicate_current_proposals() {
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(answer()),
        RuntimePolicy::default(),
    )
    .await;
    let mut req = request();
    req.tools.push(tool());
    req.tool_choice = ToolChoice::Auto;
    req.messages.push(Message::Assistant {
        text: String::new(),
        calls: vec![proposal()],
    });
    req.messages.push(Message::ToolResult {
        call_id: "call-a".into(),
        content: "done".into(),
        is_error: false,
    });
    for ids in [vec!["call-a"], vec!["call-b", "call-b"]] {
        let calls=ids.iter().map(|id|json!({"id":id,"type":"function","function":{"name":"calculate","arguments":"{\"number\":2}"}})).collect::<Vec<_>>();
        let result = json!({"choices":[{"message":{"role":"assistant","content":null,"tool_calls":calls},"finish_reason":"tool_calls"}]});
        *fixture.server.state.reply.lock().unwrap() = Reply::json(result);
        assert_eq!(
            fixture
                .runtime
                .complete("llm", req.clone(), CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::InvalidResponse
        );
    }
    assert_eq!(fixture.server.count(), 2);
}
#[tokio::test]
async fn anthropic_tool_input_stays_native_and_truncation_does_not_release_a_proposal() {
    let value = json!({"id":"message-b","type":"message","role":"assistant","content":[{"type":"tool_use","id":"call-new","name":"calculate","input":{"number":7}}],"stop_reason":"tool_use","usage":{"input_tokens":6,"output_tokens":3}});
    let fixture = Fixture::new(
        "anthropic",
        Reply::json(value.clone()),
        RuntimePolicy::default(),
    )
    .await;
    let mut req = request();
    req.tools.push(tool());
    req.tool_choice = ToolChoice::Required;
    let output = fixture
        .runtime
        .complete("llm", req.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(output.calls[0].arguments, json!({"number":7}));
    let mut invalid = value.clone();
    invalid["content"][0]["input"] = json!("{\"number\":7}");
    *fixture.server.state.reply.lock().unwrap() = Reply::json(invalid);
    assert_eq!(
        fixture
            .runtime
            .complete("llm", req.clone(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidResponse
    );
    let mut incomplete = value;
    incomplete["stop_reason"] = json!("max_tokens");
    *fixture.server.state.reply.lock().unwrap() = Reply::json(incomplete);
    let error = fixture
        .runtime
        .complete("llm", req, CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::OutputLimit);
    assert_eq!(error.usage.unwrap().output_tokens, Some(3));
    assert_eq!(fixture.server.count(), 3);
}

#[tokio::test]
async fn openai_actual_request_preserves_paired_history_native_schema_and_snapshot() {
    let result = json!({"id":"call-response","choices":[{"message":{"role":"assistant","content":null,"tool_calls":[{"id":"call-b","type":"function","function":{"name":"calculate","arguments":"{\"number\":7}"}}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":20,"completion_tokens":4,"prompt_tokens_details":{"cached_tokens":10}}});
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(result),
        RuntimePolicy::default(),
    )
    .await;
    let mut req = request();
    req.tools.push(tool());
    req.tool_choice = ToolChoice::Named("calculate".into());
    req.messages.push(Message::Assistant {
        text: String::new(),
        calls: vec![proposal()],
    });
    req.messages.push(Message::ToolResult {
        call_id: "call-a".into(),
        content: "saved result".into(),
        is_error: false,
    });
    let output = fixture
        .runtime
        .complete("llm", req.clone(), CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(output.finish, Finish::ToolCalls);
    assert_eq!(output.calls[0].arguments, json!({"number":7}));
    assert_eq!(output.usage.input_tokens, Some(20));
    assert_eq!(output.usage.cache_read_tokens, Some(10));
    assert_eq!(output.configuration_revision, 1);
    assert_eq!(output.model_id, "fixture-model");
    let received = fixture.server.state.received.lock().unwrap();
    let (headers, body) = &received[0];
    assert_eq!(headers["authorization"], "Bearer local-fixture-credential");
    assert_eq!(
        body["messages"][2]["tool_calls"][0]["function"]["arguments"],
        "{\"number\":100}"
    );
    assert_eq!(body["messages"][3]["tool_call_id"], "call-a");
    assert_eq!(
        body["tools"][0]["function"]["parameters"],
        req.tools[0].input_schema
    );
    assert_eq!(body["tool_choice"]["function"]["name"], "calculate");
    assert_eq!(body["max_tokens"], 128);
    assert_eq!(body["stream"], false);
    assert!(!body.to_string().contains("local-fixture-credential"));
}
#[tokio::test]
async fn readonly_runtime_context_stays_in_place_for_both_protocols() {
    for (api, result) in [
        (
            "openai_compatible",
            json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"OK"}}]}),
        ),
        (
            "anthropic",
            json!({"id":"message-runtime","type":"message","role":"assistant","content":[{"type":"text","text":"OK"}],"stop_reason":"end_turn"}),
        ),
    ] {
        let fixture = Fixture::new(api, Reply::json(result), RuntimePolicy::default()).await;
        let mut req = request();
        req.messages.push(Message::Assistant {
            text: "Earlier operation completed".into(),
            calls: vec![],
        });
        req.messages.push(Message::RuntimeData {
            text: "Read-only checkpoint: revision 2, not a new task".into(),
        });
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .unwrap();
        let received = fixture.server.state.received.lock().unwrap();
        let messages = received[0].1["messages"].as_array().unwrap();
        let runtime = messages.last().unwrap();
        assert_eq!(runtime["role"], "user");
        assert!(runtime["content"].to_string().contains("revision 2"));
        assert_eq!(messages[messages.len() - 2]["role"], "assistant");
    }
}
#[tokio::test]
async fn compatible_explicit_upstream_default_omits_both_output_caps() {
    let result =
        json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":"OK"}}]});
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(result),
        RuntimePolicy::default(),
    )
    .await;
    let mut provider = fixture.models.catalog()["providers"][0].clone();
    provider["output_token_parameter"] = json!("omit");
    fixture.models.save_provider(provider, None).unwrap();
    fixture
        .runtime
        .complete("llm", request(), CancellationToken::new())
        .await
        .unwrap();
    let received = fixture.server.state.received.lock().unwrap();
    let body = &received[0].1;
    assert!(body.get("max_tokens").is_none());
    assert!(body.get("max_completion_tokens").is_none());
    assert!(body.get("omit").is_none());
    assert_eq!(body["reasoning_effort"], "none");
}
#[tokio::test]
async fn anthropic_actual_request_maps_native_blocks_and_groups_multiple_tool_results() {
    let result = json!({"id":"message-a","type":"message","role":"assistant","content":[{"type":"text","text":"Answer"}],"stop_reason":"end_turn","usage":{"input_tokens":10,"output_tokens":3,"cache_read_input_tokens":5,"cache_creation_input_tokens":2}});
    let fixture = Fixture::new("anthropic", Reply::json(result), RuntimePolicy::default()).await;
    let mut req = request();
    req.tools.push(tool());
    req.tool_choice = ToolChoice::Auto;
    let mut second = proposal();
    second.id = "call-b".into();
    second.arguments = json!({"number":9});
    req.messages.push(Message::Assistant {
        text: String::new(),
        calls: vec![proposal(), second],
    });
    req.messages.push(Message::ToolResult {
        call_id: "call-a".into(),
        content: "one".into(),
        is_error: false,
    });
    req.messages.push(Message::ToolResult {
        call_id: "call-b".into(),
        content: "two".into(),
        is_error: true,
    });
    let output = fixture
        .runtime
        .complete("llm", req, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(output.text, "Answer");
    assert_eq!(output.usage.cache_write_tokens, Some(2));
    let received = fixture.server.state.received.lock().unwrap();
    let (headers, body) = &received[0];
    assert_eq!(headers["x-api-key"], "local-fixture-credential");
    assert_eq!(headers["anthropic-version"], "2023-06-01");
    assert_eq!(body["system"][0]["text"], "Instructions only.");
    assert_eq!(
        body["messages"][1]["content"][0]["input"],
        json!({"number":100})
    );
    assert_eq!(body["messages"][2]["content"].as_array().unwrap().len(), 2);
    assert_eq!(body["messages"][2]["content"][1]["is_error"], true);
}
#[tokio::test]
async fn incomplete_length_empty_unknown_tools_and_invalid_json_never_return_executable_proposals()
{
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(answer()),
        RuntimePolicy::default(),
    )
    .await;
    let mut cases = Vec::new();
    let mut value = answer();
    value["choices"][0]["finish_reason"] = json!("length");
    cases.push((value, ErrorKind::OutputLimit));
    let mut value = answer();
    value["choices"][0]["finish_reason"] = json!(null);
    cases.push((value, ErrorKind::Incomplete));
    let mut value = answer();
    value["choices"][0]["message"]["content"] = json!("   ");
    cases.push((value, ErrorKind::Empty));
    for (value, expected) in cases {
        *fixture.server.state.reply.lock().unwrap() = Reply::json(value);
        let error = fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.kind, expected);
        assert_eq!(error.usage.unwrap().input_tokens, Some(12));
    }
    let mut req = request();
    req.tools.push(tool());
    req.tool_choice = ToolChoice::Auto;
    for (name, args, diagnostic) in [
        ("unregistered", "{}", "tool_name_not_loaded"),
        (
            "calculate",
            "{\"number\":1,\"number\":2}",
            "tool_arguments_invalid_json",
        ),
        ("calculate", "\"encoded\"", "tool_arguments_not_object"),
        ("calculate", "{\"number\":", "tool_arguments_invalid_json"),
    ] {
        let result = json!({"choices":[{"message":{"role":"assistant","tool_calls":[{"id":"call-c","type":"function","function":{"name":name,"arguments":args}}]},"finish_reason":"tool_calls"}]});
        *fixture.server.state.reply.lock().unwrap() = Reply::json(result);
        let error = fixture
            .runtime
            .complete("llm", req.clone(), CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::InvalidResponse);
        assert_eq!(error.response_diagnostic, Some(diagnostic));
        assert!(error.binding.unwrap()["provider"].is_string());
    }
    assert_eq!(fixture.server.count(), 7);
}
#[tokio::test]
async fn invalid_history_and_unsupported_settings_are_rejected_before_network() {
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(answer()),
        RuntimePolicy::default(),
    )
    .await;
    let mut req = request();
    req.messages.push(Message::Assistant {
        text: "Proposal".into(),
        calls: vec![proposal()],
    });
    assert_eq!(
        fixture
            .runtime
            .complete("llm", req.clone(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidRequest
    );
    req.messages.push(Message::User {
        text: "interrupt before result".into(),
    });
    assert!(
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .is_err()
    );
    let mut req = request();
    req.messages.push(Message::ToolResult {
        call_id: "orphan".into(),
        content: "bad".into(),
        is_error: false,
    });
    assert!(
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .is_err()
    );
    let mut req = request();
    req.tool_choice = ToolChoice::Required;
    assert!(
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .is_err()
    );
    let mut req = request();
    req.output_token_limit = 513;
    assert!(
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .is_err()
    );
    let mut entry = fixture.models.catalog()["models"][0].clone();
    entry["thinking"] = json!("qwen");
    fixture.models.save_model(entry, None).unwrap();
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Unsupported
    );
    fixture
        .models
        .set_model_enabled("llm", false, None)
        .unwrap();
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Configuration
    );
    assert_eq!(fixture.server.count(), 0);
}
