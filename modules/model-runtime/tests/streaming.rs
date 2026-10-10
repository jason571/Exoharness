#[allow(dead_code)]
mod support;
use exoharness_model_runtime::{
    CancellationToken, ErrorKind, RuntimePolicy, TextSink, Tool, ToolChoice,
};
use serde_json::{Value, json};
use std::{future::Future, pin::Pin, time::Duration};
use support::{Fixture, request, server::Reply};
#[derive(Default)]
struct Sink(Vec<String>);
impl TextSink for Sink {
    fn push<'a>(
        &'a mut self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        self.0.push(text.into());
        Box::pin(async { Ok(()) })
    }
}
fn frame(value: Value) -> String {
    format!("data: {value}\r\n\r\n")
}
fn delta(text: &str) -> String {
    frame(
        json!({"id":"stream-1","choices":[{"index":0,"delta":{"content":text},"finish_reason":null}]}),
    )
}
fn ending(reason: &str) -> String {
    format!(
        "{}{}data: [DONE]\n\n",
        frame(json!({"id":"stream-1","choices":[{"index":0,"delta":{},"finish_reason":reason}]})),
        frame(json!({"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":2}}))
    )
}
fn reply(body: String) -> Reply {
    let mut r = Reply::json(Value::Null);
    r.content_type = "text/event-stream";
    r.body = body;
    r.chunked = true;
    r
}
#[tokio::test]
async fn streaming_upstream_output_default_omits_caps_and_retains_finish_validation() {
    let fixture = Fixture::new(
        "openai_compatible",
        reply(delta("OK") + &ending("stop")),
        RuntimePolicy::default(),
    )
    .await;
    let mut provider = fixture.models.catalog()["providers"][0].clone();
    provider["output_token_parameter"] = json!("omit");
    fixture.models.save_provider(provider, None).unwrap();
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let result = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut Sink::default(),
        )
        .await
        .unwrap();
    assert_eq!(result.text, "OK");
    {
        let received = fixture.server.state.received.lock().unwrap();
        let body = &received[0].1;
        assert!(body.get("max_tokens").is_none());
        assert!(body.get("max_completion_tokens").is_none());
        assert_eq!(body["stream"], true);
        assert_eq!(body["reasoning_effort"], "none");
    }
    *fixture.server.state.reply.lock().unwrap() = reply(delta("unfinished") + &ending("length"));
    let error = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut Sink::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::OutputLimit);
}
#[tokio::test]
async fn streams_answer_before_eof_with_explicit_thinking_off_and_actual_usage() {
    struct EarlySink {
        cancel: CancellationToken,
        seen: bool,
    }
    impl TextSink for EarlySink {
        fn push<'a>(
            &'a mut self,
            text: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
            assert_eq!(text, "你好");
            self.seen = true;
            self.cancel.cancel();
            Box::pin(async { Ok(()) })
        }
    }
    let body = delta("你好") + &delta(&"后续内容".repeat(100)) + &ending("stop");
    let fixture = Fixture::new("openai_compatible", reply(body), RuntimePolicy::default()).await;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let cancel = CancellationToken::new();
    let mut early = EarlySink {
        cancel: cancel.clone(),
        seen: false,
    };
    let error = fixture
        .runtime
        .complete_revision_stream("llm", revision, request(), cancel, &mut early)
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert!(early.seen, "must deliver before the remaining response/EOF");
    *fixture.server.state.reply.lock().unwrap() =
        reply(delta("你好") + &delta("，世界") + &ending("stop"));
    let mut sink = Sink::default();
    let result = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut sink,
        )
        .await
        .unwrap();
    assert_eq!(result.text, "你好，世界");
    assert_eq!(sink.0.concat(), result.text);
    assert_eq!(sink.0.len(), 2);
    assert_eq!(result.usage.input_tokens, Some(12));
    let received = fixture.server.state.received.lock().unwrap();
    for (_, body) in received.iter() {
        assert_eq!(body["stream"], true);
        assert_eq!(body["stream_options"]["include_usage"], true);
        assert_eq!(body["reasoning_effort"], "none");
    }
}
#[tokio::test]
async fn tool_fragments_are_reconstructed_once_as_native_json_not_answer_text() {
    let body = frame(
        json!({"choices":[{"index":0,"delta":{"reasoning_content":"private thinking","tool_calls":[{"index":0,"id":"call-1","type":"function","function":{"name":"calculate","arguments":"{\"number\":"}}]}}]}),
    ) + &frame(
        json!({"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"100}"}}]}}]}),
    ) + &ending("tool_calls");
    let fixture = Fixture::new("openai_compatible", reply(body), RuntimePolicy::default()).await;
    let mut req = request();
    req.tools.push(Tool {
        name: "calculate".into(),
        description: "calculate".into(),
        input_schema: json!({"type":"object"}),
    });
    req.tool_choice = ToolChoice::Required;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let mut sink = Sink::default();
    let result = fixture
        .runtime
        .complete_revision_stream("llm", revision, req, CancellationToken::new(), &mut sink)
        .await
        .unwrap();
    assert_eq!(result.calls.len(), 1);
    assert_eq!(result.calls[0].arguments, json!({"number":100}));
    assert!(sink.0.is_empty());
}
#[tokio::test]
async fn premature_eof_length_and_invalid_json_never_become_success_or_retry() {
    for (body, kind) in [
        (delta("partial"), ErrorKind::Incomplete),
        (delta("partial") + &ending("length"), ErrorKind::OutputLimit),
        (
            delta("partial") + "data: {invalid}\n\n",
            ErrorKind::InvalidResponse,
        ),
    ] {
        let fixture =
            Fixture::new("openai_compatible", reply(body), RuntimePolicy::default()).await;
        let revision = fixture.models.bind_enabled("llm").unwrap().revision;
        let mut sink = Sink::default();
        let error = fixture
            .runtime
            .complete_revision_stream(
                "llm",
                revision,
                request(),
                CancellationToken::new(),
                &mut sink,
            )
            .await
            .unwrap_err();
        assert_eq!(error.kind, kind);
        assert_eq!(sink.0, vec!["partial"]);
        assert_eq!(fixture.server.count(), 1);
    }
}
#[tokio::test]
async fn bounded_stream_and_failed_delivery_stop_without_hidden_attempts() {
    struct Reject;
    impl TextSink for Reject {
        fn push<'a>(
            &'a mut self,
            _: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
            Box::pin(async { Err(()) })
        }
    }
    let fixture = Fixture::new(
        "openai_compatible",
        reply(delta("text") + &ending("stop")),
        RuntimePolicy::default(),
    )
    .await;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let error = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut Reject,
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Cancelled);
    assert_eq!(fixture.server.count(), 1);
    let fixture = Fixture::new(
        "openai_compatible",
        reply(delta(&"x".repeat(1300)) + &ending("stop")),
        RuntimePolicy {
            response_bytes: 1024,
            ..Default::default()
        },
    )
    .await;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let error = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut Sink::default(),
        )
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::OutputLimit);
}
#[tokio::test]
async fn anthropic_stream_uses_message_stop_merges_usage_and_disables_thinking() {
    let body = [
        json!({"type":"message_start","message":{"type":"message","role":"assistant","id":"a1","content":[],"usage":{"input_tokens":7,"output_tokens":0}}}),
        json!({"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}),
        json!({"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"中文回答"}}),
        json!({"type":"content_block_stop","index":0}),
        json!({"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":3}}),
        json!({"type":"message_stop"}),
    ].into_iter().map(frame).collect::<String>();
    let fixture = Fixture::new("anthropic", reply(body), RuntimePolicy::default()).await;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let mut sink = Sink::default();
    let result = fixture
        .runtime
        .complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut sink,
        )
        .await
        .unwrap();
    assert_eq!(sink.0, vec!["中文回答"]);
    assert_eq!(result.usage.input_tokens, Some(7));
    assert_eq!(result.usage.output_tokens, Some(3));
    assert_eq!(
        fixture.server.state.received.lock().unwrap()[0].1["thinking"]["type"],
        "disabled"
    );
}
#[tokio::test]
async fn non_streaming_fallback_is_validated_without_fabricating_deltas() {
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(support::server::answer()),
        RuntimePolicy::default(),
    )
    .await;
    let revision = fixture.models.bind_enabled("llm").unwrap().revision;
    let mut sink = Sink::default();
    let result = tokio::time::timeout(
        Duration::from_secs(2),
        fixture.runtime.complete_revision_stream(
            "llm",
            revision,
            request(),
            CancellationToken::new(),
            &mut sink,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(result.text, "OK");
    assert!(sink.0.is_empty());
}
