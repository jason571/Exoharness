#[allow(dead_code)]
mod support;
use exoharness_model_runtime::{CancellationToken, ErrorKind, RuntimePolicy};
use serde_json::json;
use std::time::Duration;
use support::{
    Fixture, request,
    server::{Reply, answer},
};

async fn started(fixture: &Fixture) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.server.count() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn http_rate_limits_are_single_attempt_and_provider_error_bodies_are_never_exposed() {
    let mut reply = Reply::json(json!({"error":"private fixture value must not escape"}));
    reply.status = axum::http::StatusCode::TOO_MANY_REQUESTS;
    reply.retry_after = Some("60");
    let fixture = Fixture::new("openai_compatible", reply, RuntimePolicy::default()).await;
    let error = fixture
        .runtime
        .complete("llm", request(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Http);
    assert_eq!(error.status, Some(429));
    assert_eq!(error.retry_after_seconds, Some(60));
    assert!(!format!("{error:?}").contains("private fixture"));
    assert_eq!(fixture.server.count(), 1);
}
#[tokio::test]
async fn actual_http_timeout_and_cancel_do_not_spawn_retries_or_leak_capacity() {
    let mut reply = Reply::json(answer());
    reply.delay = Duration::from_secs(2);
    let policy = RuntimePolicy {
        timeout: Duration::from_millis(300),
        concurrency: 1,
        queued: 1,
        ..RuntimePolicy::default()
    };
    let fixture = Fixture::new("openai_compatible", reply, policy).await;
    let error = fixture
        .runtime
        .complete("llm", request(), CancellationToken::new())
        .await
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Timeout);
    assert_eq!(fixture.server.count(), 1);
    let runtime = fixture.runtime.clone();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let task = tokio::spawn(async move { runtime.complete("llm", request(), token).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        while fixture.server.count() < 2 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    cancel.cancel();
    assert_eq!(task.await.unwrap().unwrap_err().kind, ErrorKind::Cancelled);
    *fixture.server.state.reply.lock().unwrap() = Reply::json(answer());
    assert!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .is_ok()
    );
    assert_eq!(fixture.server.count(), 3);
}
#[tokio::test]
async fn bounded_queue_can_be_cancelled_and_core_close_rejects_stale_runtime_handles() {
    let mut reply = Reply::json(answer());
    reply.delay = Duration::from_secs(3);
    let fixture = Fixture::new(
        "openai_compatible",
        reply,
        RuntimePolicy {
            concurrency: 1,
            queued: 1,
            ..RuntimePolicy::default()
        },
    )
    .await;
    let runtime = fixture.runtime.clone();
    let first = tokio::spawn(async move {
        runtime
            .complete("llm", request(), CancellationToken::new())
            .await
    });
    started(&fixture).await;
    let runtime = fixture.runtime.clone();
    let cancel = CancellationToken::new();
    let token = cancel.clone();
    let second = tokio::spawn(async move { runtime.complete("llm", request(), token).await });
    tokio::time::timeout(Duration::from_secs(1), async {
        while fixture.runtime.capacity().queued != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Busy
    );
    cancel.cancel();
    assert_eq!(
        second.await.unwrap().unwrap_err().kind,
        ErrorKind::Cancelled
    );
    tokio::time::timeout(Duration::from_secs(1), fixture.runtime.close())
        .await
        .unwrap();
    assert_eq!(first.await.unwrap().unwrap_err().kind, ErrorKind::Cancelled);
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert_eq!(fixture.server.count(), 1);
}
#[tokio::test]
async fn response_limits_cover_content_length_and_chunked_bodies_and_invalid_json() {
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(answer()),
        RuntimePolicy {
            response_bytes: 1024,
            ..RuntimePolicy::default()
        },
    )
    .await;
    for chunked in [false, true] {
        let mut value = answer();
        value["choices"][0]["message"]["content"] = json!("x".repeat(2048));
        let mut reply = Reply::json(value);
        reply.chunked = chunked;
        *fixture.server.state.reply.lock().unwrap() = reply;
        assert_eq!(
            fixture
                .runtime
                .complete("llm", request(), CancellationToken::new())
                .await
                .unwrap_err()
                .kind,
            ErrorKind::OutputLimit
        );
    }
    let mut reply = Reply::json(answer());
    reply.body = "{\"choices\":[],\"choices\":[]}".into();
    *fixture.server.state.reply.lock().unwrap() = reply;
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidResponse
    );
    assert_eq!(fixture.server.count(), 3);
}
#[tokio::test]
async fn explicit_pre_cancel_and_oversized_request_do_not_submit_http() {
    let fixture = Fixture::new(
        "openai_compatible",
        Reply::json(answer()),
        RuntimePolicy {
            request_bytes: 1024,
            ..RuntimePolicy::default()
        },
    )
    .await;
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert_eq!(
        fixture
            .runtime
            .complete("llm", request(), cancel)
            .await
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    let mut req = request();
    req.messages.push(exoharness_model_runtime::Message::User {
        text: "x".repeat(2000),
    });
    assert_eq!(
        fixture
            .runtime
            .complete("llm", req, CancellationToken::new())
            .await
            .unwrap_err()
            .kind,
        ErrorKind::InvalidRequest
    );
    assert_eq!(fixture.server.count(), 0);
}
