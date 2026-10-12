#[allow(dead_code)]
mod support;
use exoharness_model_runtime::{CancellationToken, ErrorKind, RuntimePolicy};
use std::{sync::Arc, time::Duration};
use support::{
    Fixture, request,
    server::{Reply, answer},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn high_load_256_requests_keep_eight_active_cancel_queued_work_and_release_capacity() {
    let mut reply = Reply::json(answer());
    reply.delay = Duration::from_millis(80);
    let fixture = Fixture::new(
        "openai_compatible",
        reply,
        RuntimePolicy {
            concurrency: 8,
            queued: 256,
            timeout: Duration::from_secs(10),
            ..Default::default()
        },
    )
    .await;
    let barrier = Arc::new(tokio::sync::Barrier::new(257));
    let tokens: Vec<_> = (0..256).map(|_| CancellationToken::new()).collect();
    let tasks: Vec<_> = tokens
        .iter()
        .map(|token| {
            let (runtime, token, barrier) =
                (fixture.runtime.clone(), token.clone(), barrier.clone());
            tokio::spawn(async move {
                barrier.wait().await;
                runtime.complete("llm", request(), token).await
            })
        })
        .collect();
    barrier.wait().await;
    tokio::time::timeout(Duration::from_secs(3), async {
        while fixture.runtime.capacity().queued < 128 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(fixture.runtime.capacity().active, 8);
    for token in &tokens {
        token.cancel();
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap_err().kind, ErrorKind::Cancelled);
    }
    assert!(
        fixture.server.count() <= 8,
        "queued cancellation must not submit new HTTP requests"
    );
    assert_eq!(fixture.runtime.capacity().active, 0);
    assert_eq!(fixture.runtime.capacity().queued, 0);
    *fixture.server.state.reply.lock().unwrap() = Reply::json(answer());
    assert!(
        fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .is_ok()
    );
    fixture.runtime.close().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn high_load_128_successful_requests_are_neither_dropped_nor_duplicated() {
    let mut reply = Reply::json(answer());
    reply.delay = Duration::from_millis(20);
    let fixture = Fixture::new(
        "openai_compatible",
        reply,
        RuntimePolicy {
            concurrency: 8,
            queued: 128,
            timeout: Duration::from_secs(10),
            ..Default::default()
        },
    )
    .await;
    let mut tasks = Vec::new();
    for _ in 0..128 {
        let runtime = fixture.runtime.clone();
        tasks.push(tokio::spawn(async move {
            runtime
                .complete("llm", request(), CancellationToken::new())
                .await
        }));
    }
    for task in tasks {
        assert_eq!(task.await.unwrap().unwrap().text, "OK");
    }
    assert_eq!(fixture.server.count(), 128);
    assert_eq!(fixture.runtime.capacity().active, 0);
    assert_eq!(fixture.runtime.capacity().queued, 0);
    fixture.runtime.close().await;
}

#[tokio::test]
async fn provider_fault_matrix_keeps_errors_private_and_releases_capacity() {
    for status in [401, 403, 408, 413, 429, 500, 502, 503, 504] {
        let mut reply = Reply::json(serde_json::json!({"error":"private upstream diagnostic"}));
        reply.status = axum::http::StatusCode::from_u16(status).unwrap();
        let fixture = Fixture::new("openai_compatible", reply, RuntimePolicy::default()).await;
        let error = fixture
            .runtime
            .complete("llm", request(), CancellationToken::new())
            .await
            .unwrap_err();
        assert_eq!(error.status, Some(status));
        assert!(!format!("{error:?}").contains("private upstream"));
        assert_eq!(fixture.server.count(), 1);
        assert_eq!(fixture.runtime.capacity().active, 0);
        *fixture.server.state.reply.lock().unwrap() = Reply::json(answer());
        assert!(
            fixture
                .runtime
                .complete("llm", request(), CancellationToken::new())
                .await
                .is_ok()
        );
        fixture.runtime.close().await;
    }
}
