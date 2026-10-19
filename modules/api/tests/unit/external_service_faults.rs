use super::*;

#[tokio::test]
async fn changed_schema_replans_before_submission_without_repinning_or_calling_upstream() {
    for enum_widening in [false, true] {
        let live_schema = json!({"type":"object","properties":{"text":{"type":"string","enum":["general","news"],"description":"Expanded categories"}},"required":["text"]});
        let remote = mcp_server::server(mcp_server::StateData {
            echo_input_schema: enum_widening.then_some(live_schema),
            ..Default::default()
        })
        .await;
        let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
        let f=fixture(vec![tool("activate_tools",json!({"names":[name]})),tool(&name,json!({"text":"general"})),
            tool("activate_tools",json!({"names":["sandbox_run"]})),
            tool("sandbox_run",json!({"runtime":"python","entrypoint":"main.py","files":{"main.py":"from pathlib import Path\nPath('/work/final.txt').write_text('authorized fallback')\n"},"output_files":["final.txt"]})),answer()],true).await;
        register_mcp(&f, &remote.url).await;
        let registry = f.tools.registry.clone();
        let frozen = tokio::task::spawn_blocking(move || {
            let id = "mcp_server:global:_:fixture";
            let record = registry
                .0
                .capability(&RegistryAccess::Administrator, id)
                .unwrap();
            let mut cached = registry
                .0
                .discovered_tools(&RegistryAccess::Administrator, id)
                .unwrap();
            let schema =
                &mut cached.iter_mut().find(|v| v["name"] == "echo").unwrap()["inputSchema"];
            if enum_widening {
                // The production Tavily incident: const general became enum
                // general/news plus documentation changes after registration.
                schema["properties"]["text"] =
                    json!({"type":"string","const":"general","description":"Original category"});
            } else {
                schema["properties"]["legacy_optional"] = json!({"type":"string"});
            }
            registry
                .0
                .observe_capability(
                    &RegistryAccess::Administrator,
                    id,
                    record.revision,
                    Some(&cached),
                    &json!({}),
                    "admin",
                )
                .unwrap();
            (record.revision, cached)
        })
        .await
        .unwrap();
        let (session, run) = start(&f).await;
        assert_eq!(terminal(&f, &run).await, RunState::Succeeded);
        let rejected = invocation(&f, &session, &run, 2).await.result.unwrap();
        assert_eq!(rejected["_host_service_unavailable"], 1);
        assert_eq!(rejected["code_executed"], false);
        assert_eq!(
            remote
                .state
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|v| v["method"] == "tools/call")
                .count(),
            0
        );
        assert_eq!(f.upstream.requests.lock().unwrap().len(), 5);
        let registry = f.tools.registry.clone();
        tokio::task::spawn_blocking(move || {
            let id = "mcp_server:global:_:fixture";
            assert_eq!(
                registry
                    .0
                    .capability(&RegistryAccess::Administrator, id)
                    .unwrap()
                    .revision,
                frozen.0
            );
            assert_eq!(
                registry
                    .0
                    .discovered_tools(&RegistryAccess::Administrator, id)
                    .unwrap(),
                frozen.1
            );
        })
        .await
        .unwrap();
        f.tasks.close().await;
        f.sandbox.stop().await.unwrap();
        f.server.abort();
    }
}
use std::collections::BTreeMap;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn high_load_32_api_runs_queue_without_extra_model_calls_and_complete_after_release() {
    let f = Arc::new(fixture((0..32).map(|_| answer()).collect(), true).await);
    super::run_admission::configure(&f, 4, 8).await;
    f.upstream.paused.store(true, Ordering::SeqCst);
    let mut starts = tokio::task::JoinSet::new();
    for _ in 0..32 {
        let f = f.clone();
        starts.spawn(async move { start_prompt(&f, "Only reply OK; do not execute tools").await });
    }
    let mut runs = Vec::new();
    while let Some(result) = starts.join_next().await {
        runs.push(result.unwrap());
    }
    tokio::time::timeout(Duration::from_secs(5), async {
        while f.upstream.requests.lock().unwrap().len() < 4 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 4);
    let store = f.store.clone();
    let status = tokio::task::spawn_blocking(move || store.scheduler_status().unwrap())
        .await
        .unwrap();
    assert_eq!(status["active"], 4);
    assert_eq!(status["waiting"], 28);
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(
        f.upstream.requests.lock().unwrap().len(),
        4,
        "queued runs must not spend model requests"
    );
    f.upstream.paused.store(false, Ordering::SeqCst);
    for (_, run) in &runs {
        assert_eq!(terminal(&f, run).await, RunState::Succeeded);
    }
    assert_eq!(f.upstream.requests.lock().unwrap().len(), 32);
    assert!(f.upstream.replies.lock().unwrap().is_empty());
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

async fn check(config: mcp_server::StateData, succeeded: bool, retries: usize, polls: usize) {
    let remote = mcp_server::server(config).await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let mut replies = vec![
        tool("activate_tools", json!({"names":[name]})),
        tool(&name, json!({"text":"submit once"})),
    ];
    if succeeded {
        replies.push(answer());
    }
    let f = fixture(replies, true).await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    assert_eq!(
        terminal(&f, &run).await,
        if succeeded {
            RunState::Succeeded
        } else {
            RunState::Failed
        }
    );
    let requests = remote.state.requests.lock().unwrap().clone();
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
        polls
    );
    let store = f.store.clone();
    let (sid, rid) = (session.clone(), run.clone());
    let events =
        tokio::task::spawn_blocking(move || store.events("user", &sid, &rid, 0, 1000).unwrap())
            .await
            .unwrap();
    assert_eq!(events.iter().filter(|e|matches!(&e.event,exoharness_storage_postgres::ExecutionEvent::Chat{payload} if payload["type"]=="mcp_background_retry")).count(),retries);
    if !succeeded {
        assert_eq!(
            invocation(&f, &session, &run, 2).await.state,
            exoharness_contracts::invocation::InvocationState::OutcomeUnknown
        );
    }
    assert_eq!(
        f.upstream.requests.lock().unwrap().len(),
        if succeeded { 3 } else { 2 }
    );
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}

fn background(statuses: Vec<u16>, read_only: bool) -> mcp_server::StateData {
    mcp_server::StateData {
        background: true,
        background_read_only: read_only,
        http_faults: Mutex::new(BTreeMap::from([(
            "tools/call:poll".into(),
            statuses.into(),
        )])),
        ..Default::default()
    }
}
#[tokio::test]
async fn read_only_status_query_recovers_two_transient_outages_without_generation_replay() {
    check(background(vec![503, 200, 502], true), true, 2, 5).await;
}
#[tokio::test]
async fn intermittent_success_never_resets_the_status_retry_budget() {
    check(background(vec![503, 200, 502, 200, 503], true), false, 2, 5).await;
}
#[tokio::test]
async fn ambiguous_non_read_only_poll_is_not_retried() {
    check(background(vec![503], false), false, 0, 1).await;
}
#[tokio::test]
async fn another_job_id_cannot_replace_the_owned_task_or_trigger_regeneration() {
    check(
        mcp_server::StateData {
            background: true,
            background_wrong_id: true,
            ..Default::default()
        },
        false,
        0,
        1,
    )
    .await;
}
#[tokio::test]
async fn rate_limited_read_only_poll_can_recover_without_duplicate_submission() {
    check(background(vec![429], true), true, 1, 5).await;
}
#[tokio::test]
async fn pending_cancel_acknowledgement_never_releases_an_unknown_invocation() {
    let remote = mcp_server::server(mcp_server::StateData {
        background: true,
        background_never_complete: true,
        background_cancel_pending: true,
        ..Default::default()
    })
    .await;
    let name = exoharness_tools::public_name("mcp_server:global:_:fixture", "echo");
    let f = fixture(
        vec![
            tool("activate_tools", json!({"names":[name]})),
            tool(&name, json!({"text":"submit once"})),
        ],
        true,
    )
    .await;
    register_mcp(&f, &remote.url).await;
    let (session, run) = start(&f).await;
    let mut started = false;
    for _ in 0..200 {
        if remote
            .state
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r["params"]["name"] == "poll")
        {
            started = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        started,
        "background ownership must be committed before cancellation"
    );
    let store = f.store.clone();
    let rid = run.clone();
    tokio::task::spawn_blocking(move || store.chat_cancel("user", &rid).unwrap())
        .await
        .unwrap();
    f.tasks.cancel(&run).await;
    for _ in 0..200 {
        if invocation(&f, &session, &run, 2).await.state
            == exoharness_contracts::invocation::InvocationState::OutcomeUnknown
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert_eq!(
        invocation(&f, &session, &run, 2).await.state,
        exoharness_contracts::invocation::InvocationState::OutcomeUnknown
    );
    assert_eq!(terminal(&f, &run).await, RunState::Cancelled);
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
    f.tasks.close().await;
    f.sandbox.stop().await.unwrap();
    f.server.abort();
}
