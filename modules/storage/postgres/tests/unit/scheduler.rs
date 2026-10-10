use super::*;
use exoharness_contracts::scheduler::{ResourcePool, SchedulerConfiguration};

fn limits(store: &PostgresStore, user: u32, global: u32) {
    let mut config = store.scheduler_configuration().unwrap();
    config.max_active_runs_per_user = user;
    config.max_global_agent_runs = global;
    store.save_scheduler_configuration(config).unwrap();
}
fn start(store: &PostgresStore, session: &str, run: &str) -> Result<RunRecord> {
    let current = store.run("owner", session, run)?;
    store.transition_run(
        "owner",
        session,
        run,
        current.version,
        RunState::Running,
        None,
    )
}
#[test]
fn defaults_are_real_and_revision_persistence_rejects_stale_or_invalid_writes() {
    let (store, url, ns) = database();
    let default = store.scheduler_configuration().unwrap();
    let expected = SchedulerConfiguration {
        updated_at: default.updated_at,
        ..Default::default()
    };
    assert_eq!(default, expected);
    let mut next = default.clone();
    next.max_active_runs_per_user = 3;
    let saved = store.save_scheduler_configuration(next).unwrap();
    assert_eq!(saved.revision, 2);
    assert!(saved.updated_at.is_some());
    let reopened = PostgresStore::connect(&url, &ns).unwrap();
    reopened.migrate().unwrap();
    assert_eq!(reopened.scheduler_configuration().unwrap(), saved);
    assert!(matches!(
        store.save_scheduler_configuration(default),
        Err(StoreError::Conflict)
    ));
    let mut bad = saved.clone();
    bad.max_global_agent_runs = 0;
    assert!(matches!(
        store.save_scheduler_configuration(bad),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(store.scheduler_configuration().unwrap(), saved);
}
#[test]
fn per_user_slots_wait_and_cancel_releases_without_changing_saved_runs() {
    let (store, _, _) = database();
    for n in 1..=3 {
        turn(
            &store,
            &format!("session-{n}"),
            &format!("run-{n}"),
            &snapshot(),
        );
    }
    start(&store, "session-1", "run-1").unwrap();
    start(&store, "session-2", "run-2").unwrap();
    let before = store.run("owner", "session-3", "run-3").unwrap();
    for _ in 0..3 {
        assert!(matches!(
            start(&store, "session-3", "run-3"),
            Err(StoreError::CapacityWaiting)
        ));
    }
    assert_eq!(
        store.run("owner", "session-3", "run-3").unwrap().version,
        before.version
    );
    store.chat_cancel("owner", "run-1").unwrap();
    start(&store, "session-3", "run-3").unwrap();
    assert_eq!(store.scheduler_status().unwrap()["active"], 2);
}
#[test]
fn global_slots_limit_distinct_users_and_lowering_limit_does_not_cancel_work() {
    let (store, _, _) = database();
    limits(&store, 8, 1);
    turn(&store, "first", "first-run", &snapshot());
    start(&store, "first", "first-run").unwrap();
    store.create_session("other", "other").unwrap();
    let current = store
        .create_turn(CreateTurn {
            owner: "other",
            session_id: "other",
            expected_session_version: 0,
            turn_id: "other-turn",
            run_id: "other-run",
            input: "test",
            snapshot: &snapshot(),
        })
        .unwrap();
    assert!(matches!(
        store.transition_run(
            "other",
            "other",
            "other-run",
            current.version,
            RunState::Running,
            None
        ),
        Err(StoreError::CapacityWaiting)
    ));
    limits(&store, 1, 1);
    assert_eq!(
        store.run("owner", "first", "first-run").unwrap().state,
        RunState::Running
    );
    store.chat_cancel("owner", "first-run").unwrap();
    store
        .transition_run(
            "other",
            "other",
            "other-run",
            current.version,
            RunState::Running,
            None,
        )
        .unwrap();
}
#[test]
fn independent_processes_cannot_overbook_global_run_capacity() {
    let (store, url, ns) = database();
    limits(&store, 8, 1);
    for n in 1..=2 {
        turn(
            &store,
            &format!("session-{n}"),
            &format!("run-{n}"),
            &snapshot(),
        );
    }
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = (1..=2)
        .map(|n| {
            let (url, ns, barrier) = (url.clone(), ns.clone(), barrier.clone());
            std::thread::spawn(move || {
                let s = PostgresStore::connect(&url, &ns).unwrap();
                barrier.wait();
                start(&s, &format!("session-{n}"), &format!("run-{n}"))
            })
        })
        .collect();
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r, Err(StoreError::CapacityWaiting)))
            .count(),
        1
    );
}
fn pools(store: &PostgresStore, assignments: &[(&str, &str)]) {
    let mut config = store.scheduler_configuration().unwrap();
    config.max_active_runs_per_user = 16;
    config.max_global_agent_runs = 16;
    config.resource_pools = vec![
        ResourcePool {
            id: "gpu".into(),
            name: "GPU".into(),
            max_concurrency: 1,
        },
        ResourcePool {
            id: "other".into(),
            name: "Other".into(),
            max_concurrency: 1,
        },
    ];
    config.mcp_resource_assignments = assignments
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    store.save_scheduler_configuration(config).unwrap();
}
fn prepare(store: &PostgresStore, n: usize, cap: &str) -> (String, String, String) {
    let (session, run, id) = (
        format!("session-{n}"),
        format!("run-{n}"),
        format!("call-{n}"),
    );
    let mut frozen = snapshot();
    frozen.capabilities[0].id = cap.into();
    turn(store, &session, &run, &frozen);
    start(store, &session, &run).unwrap();
    let mut call = intent(&id, n as i64);
    call.capability_id = cap.into();
    store
        .prepare_invocation("owner", &session, &run, &call)
        .unwrap();
    (session, run, id)
}
fn claim(
    store: &PostgresStore,
    c: &(String, String, String),
    limit: u32,
) -> Result<InvocationAdmission> {
    store.claim_mcp_invocation("owner", &c.0, &c.1, &c.2, limit)
}
fn finish(store: &PostgresStore, c: &(String, String, String), state: I) {
    store
        .resolve_invocation(
            "owner",
            &c.0,
            &c.1,
            &c.2,
            state,
            Some(json!({"receipt":"saved"})),
        )
        .unwrap();
}
#[test]
fn shared_pool_serializes_different_mcps_while_other_pools_remain_available() {
    let (store, _, _) = database();
    pools(&store, &[("a", "gpu"), ("b", "gpu"), ("c", "other")]);
    let a = prepare(&store, 1, "a");
    assert_eq!(claim(&store, &a, 8).unwrap(), InvocationAdmission::Claimed);
    let b = prepare(&store, 2, "b");
    assert_eq!(claim(&store, &b, 8).unwrap(), InvocationAdmission::Waiting);
    let c = prepare(&store, 3, "c");
    assert_eq!(claim(&store, &c, 1).unwrap(), InvocationAdmission::Claimed);
    assert_eq!(
        store
            .invocation("owner", &b.0, &b.1, &b.2)
            .unwrap()
            .attempts,
        0
    );
    finish(&store, &a, I::Succeeded);
    assert_eq!(claim(&store, &b, 8).unwrap(), InvocationAdmission::Claimed);
}
#[test]
fn remapping_and_restart_cannot_release_an_inflight_or_unknown_pool_reservation() {
    let (store, url, ns) = database();
    pools(&store, &[("a", "gpu"), ("b", "gpu")]);
    let a = prepare(&store, 1, "a");
    claim(&store, &a, 1).unwrap();
    pools(&store, &[("a", "other"), ("b", "gpu")]);
    let b = prepare(&store, 2, "b");
    let reopened = PostgresStore::connect(&url, &ns).unwrap();
    assert_eq!(
        claim(&reopened, &b, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    finish(&store, &a, I::OutcomeUnknown);
    assert!(matches!(
        claim(&reopened, &b, 1),
        Err(StoreError::OutcomeUnknown)
    ));
    finish(&reopened, &a, I::Succeeded);
    assert_eq!(
        claim(&reopened, &b, 1).unwrap(),
        InvocationAdmission::Claimed
    );
}
#[test]
fn capability_limit_is_preserved_even_when_shared_pool_has_capacity() {
    let (store, _, _) = database();
    pools(&store, &[("a", "gpu")]);
    let mut config = store.scheduler_configuration().unwrap();
    config.resource_pools[0].max_concurrency = 8;
    store.save_scheduler_configuration(config).unwrap();
    let first = prepare(&store, 1, "a");
    claim(&store, &first, 1).unwrap();
    let second = prepare(&store, 2, "a");
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    finish(&store, &first, I::Succeeded);
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Claimed
    );
}
