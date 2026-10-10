use super::*;
async fn configure(f: &Fixture, user: u32, global: u32) {
    let s = f.store.clone();
    tokio::task::spawn_blocking(move || {
        let mut c = s.scheduler_configuration().unwrap();
        c.max_active_runs_per_user = user;
        c.max_global_agent_runs = global;
        s.save_scheduler_configuration(c).unwrap();
    })
    .await
    .unwrap();
}
async fn waiting(f: &Fixture, session: &str, run: &str) {
    for _ in 0..200 {
        let (s, session, run) = (f.store.clone(), session.to_owned(), run.to_owned());
        let yes=tokio::task::spawn_blocking(move||s.events("user",&session,&run,0,256).unwrap().iter()
            .any(|e|matches!(&e.event,exoharness_storage_postgres::ExecutionEvent::Chat{payload} if payload["type"]=="resource_queue" && payload["state"]=="waiting"))).await.unwrap();
        if yes {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("Run never entered queue");
}
#[tokio::test]
async fn queued_run_makes_no_model_requests_and_starts_after_slot_release() {
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
        ],
        true,
    )
    .await;
    configure(&f, 1, 4).await;
    register_mcp(&f, &remote.url).await;
    let (_, first) = start(&f).await;
    for _ in 0..200 {
        if f.upstream.requests.lock().unwrap().len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (session, second) = start_prompt(&f, "你好，你是谁？").await;
    waiting(&f, &session, &second).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
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
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 4);
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
#[tokio::test]
async fn cancelling_queued_run_never_calls_model_or_touches_active_mcp() {
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
        ],
        true,
    )
    .await;
    configure(&f, 1, 1).await;
    register_mcp(&f, &remote.url).await;
    let (_, first) = start(&f).await;
    for _ in 0..200 {
        if f.upstream.requests.lock().unwrap().len() == 2 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let (session, second) = start_prompt(&f, "普通对话").await;
    waiting(&f, &session, &second).await;
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
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 2);
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
