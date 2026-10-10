use super::*;

#[tokio::test]
async fn cancellation_committed_before_progress_write_drains_and_persists_the_receipt() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        background_poll_delay_ms: 200,
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
    let (session, run) = start(&f).await;
    let mut poll_started = false;
    for _ in 0..200 {
        if remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["params"]["name"] == "poll")
        {
            poll_started = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(poll_started);
    // Deliberately commit cancellation before notifying the worker, as can
    // occur between the control endpoint's database write and token signal.
    let (store, saved_run) = (f.store.clone(), run.clone());
    tokio::task::spawn_blocking(move || store.chat_cancel("user", &saved_run))
        .await
        .unwrap()
        .unwrap();
    let mut settled = false;
    for _ in 0..300 {
        if invocation(&f, &session, &run, 2).await.state
            == exoharness_contracts::invocation::InvocationState::FailedAfterExecution
        {
            settled = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        settled,
        "rejected late progress must not drop the authoritative cancellation receipt"
    );
    assert_eq!(terminal(&f, &run).await, RunState::Cancelled);
    let receipt = invocation(&f, &session, &run, 2).await.result.unwrap();
    assert_eq!(receipt["structuredContent"]["status"], "cancelled");
    assert_eq!(starts(&remote), 1);
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["params"]["name"] == "cancel")
            .count(),
        1
    );
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

fn starts(remote: &mcp_server::Server) -> usize {
    remote
        .state
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|r| r["method"] == "tools/call" && r["params"]["name"] == "echo")
        .count()
}
async fn queued(f: &Fixture, session: &str, run: &str) {
    for _ in 0..200 {
        let store = f.store.clone();
        let (session, run) = (session.to_owned(), run.to_owned());
        let events = tokio::task::spawn_blocking(move || {
            store.events("user", &session, &run, 0, 256).unwrap()
        })
        .await
        .unwrap();
        if events.iter().any(|e|matches!(&e.event, exoharness_storage_postgres::ExecutionEvent::Chat {payload} if payload["type"]=="resource_queue" && payload["state"]=="waiting")) {return;}
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("second MCP invocation never queued");
}
async fn first_started(remote: &mcp_server::Server) {
    for _ in 0..200 {
        if starts(remote) == 1 {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("first MCP task did not start");
}

#[tokio::test]
async fn same_mcp_waits_through_background_completion_without_more_model_calls() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"first"})),
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"second"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, first) = start(&f).await;
    first_started(&remote).await;
    let (session, second) = start(&f).await;
    queued(&f, &session, &second).await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    assert_eq!(
        starts(&remote),
        1,
        "second call must not enter the MCP queue"
    );
    assert_eq!(
        f.upstream.requests.lock().unwrap().len(),
        4,
        "waiting must not reenter the model"
    );
    let saved = invocation(&f, &session, &second, 2).await;
    assert_eq!(
        saved.state,
        exoharness_contracts::invocation::InvocationState::Prepared
    );
    assert_eq!(saved.attempts, 0);
    // Ordinary conversation remains independent of the occupied MCP slot.
    f.upstream.replies.lock().unwrap().push_back(answer());
    let (_, ordinary) = start_prompt(&f, "你好，你是谁？").await;
    assert_eq!(terminal(&f, &ordinary).await, RunState::Succeeded);
    assert_eq!(starts(&remote), 1);
    f.upstream
        .replies
        .lock()
        .unwrap()
        .extend([answer(), answer()]);
    remote
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    assert_eq!(terminal(&f, &first).await, RunState::Succeeded);
    assert_eq!(terminal(&f, &second).await, RunState::Succeeded);
    assert_eq!(starts(&remote), 2);
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 7);
    assert_eq!(invocation(&f, &session, &second, 2).await.attempts, 1);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn cancelling_a_queued_mcp_call_never_submits_it_or_cancels_the_active_job() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"first"})),
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"second"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (_, first) = start(&f).await;
    first_started(&remote).await;
    let (session, second) = start(&f).await;
    queued(&f, &session, &second).await;
    let response = f
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/v1/runs/{second}/cancel"))
                .header("host", "127.0.0.1:9798")
                .header("authorization", format!("Bearer {TOKEN}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(terminal(&f, &second).await, RunState::Cancelled);
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(starts(&remote), 1);
    assert_eq!(
        remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r["params"]["name"] == "cancel")
            .count(),
        0
    );
    assert_eq!(invocation(&f, &session, &second, 2).await.attempts, 0);
    f.upstream.replies.lock().unwrap().push_back(answer());
    remote
        .state
        .release_background
        .store(true, Ordering::SeqCst);
    assert_eq!(terminal(&f, &first).await, RunState::Succeeded);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn disabling_mcp_while_waiting_prevents_submission_and_preserves_active_work() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"first"})),
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"second"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (first_session, first) = start(&f).await;
    first_started(&remote).await;
    let (session, second) = start(&f).await;
    queued(&f, &session, &second).await;
    let registry = f.tools.registry.clone();
    tokio::task::spawn_blocking(move || {
        let access = RegistryAccess::Administrator;
        let record = registry
            .0
            .capability(&access, "mcp_server:global:_:fixture")
            .unwrap();
        registry
            .0
            .set_capability_enabled(&access, &record.id, record.revision, false, "admin")
            .unwrap();
    })
    .await
    .unwrap();
    assert_eq!(terminal(&f, &second).await, RunState::Failed);
    assert_eq!(starts(&remote), 1);
    assert_eq!(invocation(&f, &session, &second, 2).await.attempts, 0);
    // Existing observation also obeys revocation; its external outcome remains
    // unknown rather than releasing capacity as if a cancellation were proven.
    assert_eq!(terminal(&f, &first).await, RunState::Failed);
    assert_eq!(
        invocation(&f, &first_session, &first, 2).await.state,
        exoharness_contracts::invocation::InvocationState::OutcomeUnknown
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

#[tokio::test]
async fn confirmed_active_job_cancellation_releases_the_slot_for_the_waiting_call() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"first"})),
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"second"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (first_session, first) = start(&f).await;
    first_started(&remote).await;
    let (session, second) = start(&f).await;
    queued(&f, &session, &second).await;
    for run in [&first, &second] {
        let response = f
            .router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/runs/{run}/cancel"))
                    .header("host", "127.0.0.1:9798")
                    .header("authorization", format!("Bearer {TOKEN}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(terminal(&f, run).await, RunState::Cancelled);
        if run == &first {
            for _ in 0..200 {
                if starts(&remote) == 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert_eq!(
                starts(&remote),
                2,
                "authoritative cancellation must let the waiter execute"
            );
            assert_eq!(
                invocation(&f, &first_session, &first, 2).await.state,
                exoharness_contracts::invocation::InvocationState::FailedAfterExecution
            );
        }
    }
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 4);
    let saved = invocation(&f, &session, &second, 2).await;
    assert_eq!(saved.attempts, 1);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
