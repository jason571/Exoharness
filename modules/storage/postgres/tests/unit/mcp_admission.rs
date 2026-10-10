use super::*;

fn prepared(store: &PostgresStore, number: usize, capability: &str) -> (String, String, String) {
    // These tests isolate MCP admission rather than Run admission.
    let mut config = store.scheduler_configuration().unwrap();
    if config.max_active_runs_per_user == 2 {
        config.max_active_runs_per_user = 32;
        config.max_global_agent_runs = 32;
        store.save_scheduler_configuration(config).unwrap();
    }
    let session = format!("session-{number}");
    let run = format!("run-{number}");
    let id = format!("invocation-{number}");
    let mut frozen = snapshot();
    frozen.capabilities[0].id = capability.into();
    let record = turn(store, &session, &run, &frozen);
    store
        .transition_run(
            "owner",
            &session,
            &run,
            record.version,
            RunState::Running,
            None,
        )
        .unwrap();
    let mut call = intent(&id, number as i64);
    call.capability_id = capability.into();
    store
        .prepare_invocation("owner", &session, &run, &call)
        .unwrap();
    (session, run, id)
}
fn claim(
    store: &PostgresStore,
    call: &(String, String, String),
    limit: u32,
) -> Result<InvocationAdmission> {
    store.claim_mcp_invocation("owner", &call.0, &call.1, &call.2, limit)
}
fn finish(store: &PostgresStore, call: &(String, String, String), state: I) {
    store
        .resolve_invocation(
            "owner",
            &call.0,
            &call.1,
            &call.2,
            state,
            Some(json!({"receipt":"exact"})),
        )
        .unwrap();
}

#[test]
fn single_slot_waits_across_sessions_without_consuming_attempts_then_advances() {
    let (store, _, _) = database();
    let first = prepared(&store, 1, "cap");
    assert_eq!(
        claim(&store, &first, 1).unwrap(),
        InvocationAdmission::Claimed
    );
    let second = prepared(&store, 2, "cap");
    for _ in 0..6 {
        assert_eq!(
            claim(&store, &second, 1).unwrap(),
            InvocationAdmission::Waiting
        );
    }
    let saved = store
        .invocation("owner", &second.0, &second.1, &second.2)
        .unwrap();
    assert_eq!(saved.state, I::Prepared);
    assert_eq!(saved.attempts, 0);
    finish(&store, &first, I::Succeeded);
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Claimed
    );
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Unavailable
    );
}

#[test]
fn configured_two_slots_and_unrelated_capabilities_do_not_serialize() {
    let (store, _, _) = database();
    let first = prepared(&store, 1, "cap");
    assert_eq!(
        claim(&store, &first, 2).unwrap(),
        InvocationAdmission::Claimed
    );
    let second = prepared(&store, 2, "cap");
    assert_eq!(
        claim(&store, &second, 2).unwrap(),
        InvocationAdmission::Claimed
    );
    let third = prepared(&store, 3, "cap");
    assert_eq!(
        claim(&store, &third, 2).unwrap(),
        InvocationAdmission::Waiting
    );
    let other = prepared(&store, 4, "other-cap");
    assert_eq!(
        claim(&store, &other, 1).unwrap(),
        InvocationAdmission::Claimed
    );
    finish(&store, &first, I::FailedAfterExecution);
    assert_eq!(
        claim(&store, &third, 2).unwrap(),
        InvocationAdmission::Claimed
    );
}

#[test]
fn restart_and_unknown_outcome_do_not_free_an_external_slot() {
    let (store, url, namespace) = database();
    let first = prepared(&store, 1, "cap");
    claim(&store, &first, 1).unwrap();
    let second = prepared(&store, 2, "cap");
    let reopened = PostgresStore::connect(&url, &namespace).unwrap();
    assert_eq!(
        claim(&reopened, &second, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    finish(&store, &first, I::OutcomeUnknown);
    assert!(matches!(
        claim(&reopened, &second, 1),
        Err(StoreError::OutcomeUnknown)
    ));
    finish(&reopened, &first, I::Succeeded);
    assert_eq!(
        claim(&reopened, &second, 1).unwrap(),
        InvocationAdmission::Claimed
    );
}

#[test]
fn cancelled_waiter_does_not_block_the_next_prepared_call() {
    let (store, _, _) = database();
    let first = prepared(&store, 1, "cap");
    claim(&store, &first, 1).unwrap();
    let second = prepared(&store, 2, "cap");
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    store.chat_cancel("owner", &second.1).unwrap();
    let third = prepared(&store, 3, "cap");
    finish(&store, &first, I::Succeeded);
    assert_eq!(
        claim(&store, &third, 1).unwrap(),
        InvocationAdmission::Claimed
    );
    assert_eq!(
        store
            .invocation("owner", &second.0, &second.1, &second.2)
            .unwrap()
            .attempts,
        0
    );
}

#[test]
fn independent_workers_racing_for_one_slot_commit_only_one_submission() {
    let (store, url, namespace) = database();
    let first = prepared(&store, 1, "cap");
    let second = prepared(&store, 2, "cap");
    let barrier = Arc::new(Barrier::new(2));
    let threads: Vec<_> = [first, second]
        .into_iter()
        .map(|call| {
            let (url, namespace, barrier) = (url.clone(), namespace.clone(), barrier.clone());
            std::thread::spawn(move || {
                let connection = PostgresStore::connect(&url, &namespace).unwrap();
                barrier.wait();
                claim(&connection, &call, 1).unwrap()
            })
        })
        .collect();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(
        results
            .iter()
            .filter(|&&value| value == InvocationAdmission::Claimed)
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|&&value| value == InvocationAdmission::Waiting)
            .count(),
        1
    );
}

#[test]
fn reducing_capacity_drains_existing_calls_without_cancelling_them() {
    let (store, _, _) = database();
    let first = prepared(&store, 1, "cap");
    claim(&store, &first, 2).unwrap();
    let second = prepared(&store, 2, "cap");
    claim(&store, &second, 2).unwrap();
    let third = prepared(&store, 3, "cap");
    assert_eq!(
        claim(&store, &third, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    finish(&store, &first, I::Succeeded);
    assert_eq!(
        claim(&store, &third, 1).unwrap(),
        InvocationAdmission::Waiting
    );
    finish(&store, &second, I::Succeeded);
    assert_eq!(
        claim(&store, &third, 1).unwrap(),
        InvocationAdmission::Claimed
    );
}

#[test]
fn invalid_capacity_rejects_without_submission_and_native_claim_remains_independent() {
    let (store, _, _) = database();
    let first = prepared(&store, 1, "cap");
    assert!(matches!(
        claim(&store, &first, 0),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        store
            .invocation("owner", &first.0, &first.1, &first.2)
            .unwrap()
            .attempts,
        0
    );
    assert!(
        store
            .claim_invocation("owner", &first.0, &first.1, &first.2)
            .unwrap()
    );
}

#[test]
fn verified_late_receipt_frees_capacity_without_resurrecting_archived_conversation() {
    let (store, url, namespace) = database();
    let first = prepared(&store, 1, "cap");
    claim(&store, &first, 1).unwrap();
    finish(&store, &first, I::OutcomeUnknown);
    store.chat_cancel("owner", &first.1).unwrap();
    let mut db = sql(&url, &namespace);
    db.execute(
        "UPDATE execution_sessions SET archived=1 WHERE id=$1",
        &[&first.0],
    )
    .unwrap();
    let second = prepared(&store, 2, "cap");
    assert!(matches!(
        claim(&store, &second, 1),
        Err(StoreError::OutcomeUnknown)
    ));
    assert!(matches!(
        store.resolve_invocation(
            "other-owner",
            &first.0,
            &first.1,
            &first.2,
            I::Succeeded,
            Some(json!({"late":true}))
        ),
        Err(StoreError::NotFound)
    ));
    let receipt = store
        .resolve_invocation(
            "owner",
            &first.0,
            &first.1,
            &first.2,
            I::Succeeded,
            Some(json!({"late":true})),
        )
        .unwrap();
    assert_eq!(receipt.state, I::Succeeded);
    assert_eq!(
        db.query_one(
            "SELECT archived FROM execution_sessions WHERE id=$1",
            &[&first.0]
        )
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.query_one("SELECT status FROM execution_runs WHERE id=$1", &[&first.1])
            .unwrap()
            .get::<_, String>(0),
        "cancelled"
    );
    assert!(store.owned_run("owner", &first.1).is_err());
    assert_eq!(
        claim(&store, &second, 1).unwrap(),
        InvocationAdmission::Claimed
    );
}
