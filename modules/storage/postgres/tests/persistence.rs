use exoharness_contracts::invocation::InvocationState as I;
use exoharness_storage_postgres::*;
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Barrier,
        atomic::{AtomicU64, Ordering},
    },
};
#[path = "unit/mcp_admission.rs"]
mod mcp_admission;
#[path = "unit/scheduler.rs"]
mod scheduler;
#[path = "unit/task_context.rs"]
mod task_context;

fn database() -> (PostgresStore, String, String) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let url=std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("real PostgreSQL required; set EXOHARNESS_PRO_TEST_DATABASE_URL to a disposable test database");
    let namespace = format!(
        "pro_test_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = PostgresStore::connect(&url, &namespace).unwrap();
    store.migrate().unwrap();
    (store, url, namespace)
}
fn snapshot() -> ExecutionSnapshot {
    ExecutionSnapshot {
        version: 1,
        assistant_id: "assistant".into(),
        assistant_revision: 2,
        model: ModelSnapshot {
            config_id: "model".into(),
            revision: 3,
            adapter: "mock".into(),
            settings: json!({"temperature":0}),
        },
        capabilities: vec![CapabilitySnapshot {
            id: "cap".into(),
            revision: 7,
            grant_revision: 9,
            allowed: true,
            tool_schemas: BTreeMap::from([(
                "calculate".into(),
                json!({"type":"object","required":["value"]}),
            )]),
        }],
        max_consecutive_identical_calls: 3,
        requires_execution: true,
    }
}
fn turn(
    store: &PostgresStore,
    session: &str,
    run: &str,
    snapshot: &ExecutionSnapshot,
) -> RunRecord {
    store.create_session("owner", session).unwrap();
    store
        .create_turn(CreateTurn {
            owner: "owner",
            session_id: session,
            expected_session_version: store.session_version("owner", session).unwrap(),
            turn_id: &format!("turn-{run}"),
            run_id: run,
            input: "perform task",
            snapshot,
        })
        .unwrap()
}
fn running(store: &PostgresStore) -> RunRecord {
    let run = turn(store, "session", "run", &snapshot());
    store
        .transition_run(
            "owner",
            "session",
            "run",
            run.version,
            RunState::Running,
            None,
        )
        .unwrap()
}
fn intent(id: &str, value: i64) -> InvocationIntent {
    InvocationIntent {
        id: id.into(),
        logical_key: format!("logical-{id}"),
        capability_id: "cap".into(),
        capability_revision: 7,
        tool: "calculate".into(),
        arguments: json!({"value":value}),
    }
}
fn control(
    store: &PostgresStore,
    next: RunState,
    recovery: Option<RecoveryDecision>,
) -> Result<RunRecord> {
    let current = store.run("owner", "session", "run")?;
    store.transition_run("owner", "session", "run", current.version, next, recovery)
}
fn recover(action: RecoveryAction) -> RecoveryDecision {
    RecoveryDecision {
        action,
        code: "test_pause".into(),
        message: "saved".into(),
    }
}
fn sql(url: &str, namespace: &str) -> postgres::Client {
    let mut client = postgres::Client::connect(url, postgres::NoTls).unwrap();
    client
        .batch_execute(&format!("SET search_path={namespace},public"))
        .unwrap();
    client
}

#[test]
fn legacy_schema_is_preserved_and_migration_is_repeatable() {
    let (store, url, ns) = database();
    store.migrate().unwrap();
    let mut client = sql(&url, &ns);
    let count: i64 = client
        .query_one(
            "SELECT COUNT(*) FROM information_schema.tables WHERE table_schema=$1",
            &[&ns],
        )
        .unwrap()
        .get(0);
    // Original execution/knowledge tables, resource reservations, and external Session mapping.
    assert_eq!(count, 36);
    for table in [
        "execution_runs",
        "execution_external_sessions",
        "execution_invocations",
        "execution_model_requests",
        "execution_run_artifacts",
        "execution_context_checkpoints",
        "execution_response_actions",
        "knowledge_documents",
        "knowledge_categories",
        "knowledge_libraries",
        "knowledge_library_grants",
        "personal_memories",
    ] {
        assert!(client.query_one("SELECT EXISTS(SELECT 1 FROM information_schema.tables WHERE table_schema=$1 AND table_name=$2)",&[&ns,&table]).unwrap().get::<_,bool>(0));
    }
    client.execute("INSERT INTO execution_sessions(id,title,created_at,updated_at,owner_user_id) VALUES('legacy-session','old title',1,1,'owner')",&[]).unwrap();
    store.migrate().unwrap();
    assert_eq!(
        client
            .query_one(
                "SELECT title FROM execution_sessions WHERE id='legacy-session'",
                &[]
            )
            .unwrap()
            .get::<_, String>(0),
        "old title"
    );
}

#[test]
fn owner_scope_idempotent_turn_and_per_session_concurrency() {
    let (store, _, _) = database();
    let frozen = snapshot();
    let run = turn(&store, "session", "run", &frozen);
    let repeated = store
        .create_turn(CreateTurn {
            owner: "owner",
            session_id: "session",
            expected_session_version: 0,
            turn_id: "turn-run",
            run_id: "run",
            input: "perform task",
            snapshot: &frozen,
        })
        .unwrap();
    assert_eq!(run, repeated);
    assert_eq!(
        store
            .events("owner", "session", "run", 0, 100)
            .unwrap()
            .len(),
        2
    );
    assert!(matches!(
        store.run("other", "session", "run"),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store.create_turn(CreateTurn {
            owner: "owner",
            session_id: "session",
            expected_session_version: run.version,
            turn_id: "other-turn",
            run_id: "other-run",
            input: "another task",
            snapshot: &frozen
        }),
        Err(StoreError::Conflict)
    ));
    assert!(
        turn(&store, "other-session", "other-run", &frozen)
            .state
            .active()
    );
}

#[test]
fn concurrent_claim_is_at_most_once_with_one_committed_event() {
    let (store, _, _) = database();
    running(&store);
    store
        .prepare_invocation("owner", "session", "run", &intent("inv", 1))
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let claims = std::thread::scope(|scope| {
        let workers = (0..2)
            .map(|_| {
                let store = store.clone();
                let barrier = barrier.clone();
                scope.spawn(move || {
                    barrier.wait();
                    store
                        .claim_invocation("owner", "session", "run", "inv")
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|claimed| *claimed)
            .count()
    });
    assert_eq!(claims, 1);
    assert_eq!(
        store
            .invocation("owner", "session", "run", "inv")
            .unwrap()
            .attempts,
        1
    );
    let submitted = store
        .events("owner", "session", "run", 0, 100)
        .unwrap()
        .iter()
        .filter(|e| matches!(e.event, ExecutionEvent::InvocationSubmitted { .. }))
        .count();
    assert_eq!(submitted, 1);
}

#[test]
fn unknown_outcome_survives_reconnect_and_blocks_resume_until_receipt() {
    let (store, url, ns) = database();
    running(&store);
    store
        .prepare_invocation("owner", "session", "run", &intent("inv", 1))
        .unwrap();
    assert!(
        store
            .claim_invocation("owner", "session", "run", "inv")
            .unwrap()
    );
    store
        .resolve_invocation("owner", "session", "run", "inv", I::OutcomeUnknown, None)
        .unwrap();
    control(
        &store,
        RunState::Paused,
        Some(recover(RecoveryAction::Resume)),
    )
    .unwrap();
    let reopened = PostgresStore::connect(&url, &ns).unwrap();
    assert!(matches!(
        control(&reopened, RunState::Running, None),
        Err(StoreError::OutcomeUnknown)
    ));
    reopened
        .resolve_invocation(
            "owner",
            "session",
            "run",
            "inv",
            I::Succeeded,
            Some(json!({"value":1})),
        )
        .unwrap();
    control(&reopened, RunState::Running, None).unwrap();
    assert!(
        !reopened
            .claim_invocation("owner", "session", "run", "inv")
            .unwrap()
    );
    control(&reopened, RunState::Succeeded, None).unwrap();
}

#[test]
fn identical_call_limit_survives_recovery_and_counts_actual_submissions_only() {
    let (store, url, ns) = database();
    running(&store);
    for n in 1..=3 {
        let id = format!("inv{n}");
        store
            .prepare_invocation("owner", "session", "run", &intent(&id, 1))
            .unwrap();
        assert!(
            store
                .claim_invocation("owner", "session", "run", &id)
                .unwrap()
        );
        store
            .resolve_invocation(
                "owner",
                "session",
                "run",
                &id,
                I::RejectedBeforeExecution,
                Some(json!({"invalid":true})),
            )
            .unwrap();
    }
    control(
        &store,
        RunState::Paused,
        Some(recover(RecoveryAction::Resume)),
    )
    .unwrap();
    let reopened = PostgresStore::connect(&url, &ns).unwrap();
    control(&reopened, RunState::Running, None).unwrap();
    reopened
        .prepare_invocation("owner", "session", "run", &intent("inv4", 1))
        .unwrap();
    assert!(matches!(
        reopened.claim_invocation("owner", "session", "run", "inv4"),
        Err(StoreError::RepeatedCall)
    ));
    assert_eq!(
        reopened
            .invocation("owner", "session", "run", "inv4")
            .unwrap()
            .attempts,
        0
    );
    reopened
        .prepare_invocation("owner", "session", "run", &intent("different", 2))
        .unwrap();
    assert!(
        reopened
            .claim_invocation("owner", "session", "run", "different")
            .unwrap()
    );
}

#[test]
fn cancellation_preserves_late_receipt_without_restarting_run() {
    let (store, _, _) = database();
    running(&store);
    store
        .prepare_invocation("owner", "session", "run", &intent("inv", 1))
        .unwrap();
    store
        .claim_invocation("owner", "session", "run", "inv")
        .unwrap();
    control(&store, RunState::Cancelled, None).unwrap();
    let receipt = json!({"file":"artifact://result"});
    store
        .resolve_invocation(
            "owner",
            "session",
            "run",
            "inv",
            I::Succeeded,
            Some(receipt.clone()),
        )
        .unwrap();
    let count = store
        .events("owner", "session", "run", 0, 100)
        .unwrap()
        .len();
    store
        .resolve_invocation(
            "owner",
            "session",
            "run",
            "inv",
            I::Succeeded,
            Some(receipt),
        )
        .unwrap();
    assert_eq!(
        store
            .events("owner", "session", "run", 0, 100)
            .unwrap()
            .len(),
        count
    );
    assert_eq!(
        store.run("owner", "session", "run").unwrap().state,
        RunState::Cancelled
    );
    assert!(matches!(
        store.claim_invocation("owner", "session", "run", "inv"),
        Err(StoreError::Conflict)
    ));
    assert!(matches!(
        store.resolve_invocation(
            "owner",
            "session",
            "run",
            "inv",
            I::Succeeded,
            Some(json!({"different":true}))
        ),
        Err(StoreError::Conflict)
    ));
}

#[test]
fn changed_snapshot_stale_version_and_false_execution_success_are_rejected() {
    let (store, url, ns) = database();
    let run = running(&store);
    assert!(matches!(
        control(&store, RunState::Succeeded, None),
        Err(StoreError::MissingExecution)
    ));
    assert!(matches!(
        store.transition_run(
            "owner",
            "session",
            "run",
            run.version - 1,
            RunState::Failed,
            None
        ),
        Err(StoreError::Conflict)
    ));
    control(
        &store,
        RunState::Paused,
        Some(recover(RecoveryAction::ChangeInput)),
    )
    .unwrap();
    assert!(matches!(
        control(&store, RunState::Running, None),
        Err(StoreError::Conflict)
    ));
    let mut client = sql(&url, &ns);
    let mut changed = snapshot().model;
    changed.revision = 99;
    client
        .execute(
            "UPDATE execution_runs SET model_snapshot_json=$1 WHERE id='run'",
            &[&serde_json::to_string(&changed).unwrap()],
        )
        .unwrap();
    assert!(matches!(
        store.run("owner", "session", "run"),
        Err(StoreError::Conflict)
    ));
}

#[test]
fn rejected_event_insert_rolls_back_claim_and_retry_counter_atomically() {
    let (store, url, ns) = database();
    running(&store);
    store
        .prepare_invocation("owner", "session", "run", &intent("inv", 1))
        .unwrap();
    let before = store.run("owner", "session", "run").unwrap().version;
    let mut client = sql(&url, &ns);
    client.batch_execute("CREATE FUNCTION reject_submission() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF NEW.event_type='pro_invocation_submitted' THEN RAISE EXCEPTION 'test write fault'; END IF; RETURN NEW; END $$; CREATE TRIGGER reject_submission BEFORE INSERT ON execution_events FOR EACH ROW EXECUTE FUNCTION reject_submission()").unwrap();
    assert!(matches!(
        store.claim_invocation("owner", "session", "run", "inv"),
        Err(StoreError::Database)
    ));
    let inv = store.invocation("owner", "session", "run", "inv").unwrap();
    assert_eq!(inv.state, I::Prepared);
    assert_eq!(inv.attempts, 0);
    assert_eq!(
        store.run("owner", "session", "run").unwrap().version,
        before
    );
    client
        .batch_execute("DROP TRIGGER reject_submission ON execution_events")
        .unwrap();
    assert!(
        store
            .claim_invocation("owner", "session", "run", "inv")
            .unwrap()
    );
}

#[test]
fn revision_authorization_and_native_arguments_are_enforced_before_preparation() {
    let (store, _, _) = database();
    running(&store);
    let mut call = intent("inv", 1);
    call.capability_revision = 8;
    assert!(matches!(
        store.prepare_invocation("owner", "session", "run", &call),
        Err(StoreError::NotAuthorized)
    ));
    call.capability_revision = 7;
    call.arguments = json!("{\"value\":1}");
    assert!(matches!(
        store.prepare_invocation("owner", "session", "run", &call),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        store
            .events("owner", "session", "run", 0, 100)
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn control_updates_preserve_existing_context_and_do_not_duplicate_arguments() {
    let (store, url, ns) = database();
    running(&store);
    let mut client = sql(&url, &ns);
    let raw: String = client
        .query_one(
            "SELECT context_json FROM execution_context_checkpoints WHERE run_id='run'",
            &[],
        )
        .unwrap()
        .get(0);
    let mut checkpoint: serde_json::Value = serde_json::from_str(&raw).unwrap();
    checkpoint["messages"] = json!([{"role":"user","content":"keep this exact"}]);
    checkpoint["future_field"] = json!({"value":[1,2,3]});
    client
        .execute(
            "UPDATE execution_context_checkpoints SET context_json=$1 WHERE run_id='run'",
            &[&checkpoint.to_string()],
        )
        .unwrap();
    let mut call = intent("inv", 1);
    call.arguments["payload"] = json!("unique payload ".repeat(1000));
    store
        .prepare_invocation("owner", "session", "run", &call)
        .unwrap();
    store
        .claim_invocation("owner", "session", "run", "inv")
        .unwrap();
    let raw: String = client
        .query_one(
            "SELECT context_json FROM execution_context_checkpoints WHERE run_id='run'",
            &[],
        )
        .unwrap()
        .get(0);
    let updated: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(updated["messages"], checkpoint["messages"]);
    assert_eq!(updated["future_field"], checkpoint["future_field"]);
    assert!(
        !updated["_exoharness_pro"]
            .to_string()
            .contains("unique payload")
    );
    assert_eq!(
        store
            .invocation("owner", "session", "run", "inv")
            .unwrap()
            .intent
            .arguments,
        call.arguments
    );
}

#[test]
fn all_unknown_calls_must_be_reconciled_before_resume_is_available() {
    let (store, _, _) = database();
    running(&store);
    for id in ["one", "two"] {
        store
            .prepare_invocation("owner", "session", "run", &intent(id, 1))
            .unwrap();
        store
            .claim_invocation("owner", "session", "run", id)
            .unwrap();
    }
    control(
        &store,
        RunState::Paused,
        Some(recover(RecoveryAction::Resume)),
    )
    .unwrap();
    for (id, action) in [
        ("one", RecoveryAction::ReconcileExternal),
        ("two", RecoveryAction::Resume),
    ] {
        store
            .resolve_invocation(
                "owner",
                "session",
                "run",
                id,
                I::Succeeded,
                Some(json!({"ok":true})),
            )
            .unwrap();
        let paused = store.run("owner", "session", "run").unwrap();
        assert_eq!(paused.state, RunState::Paused);
        assert_eq!(paused.recovery.unwrap().action, action);
    }
    control(&store, RunState::Running, None).unwrap();
}

#[test]
fn legacy_run_without_control_fails_closed_and_remains_unchanged() {
    let (store, url, ns) = database();
    running(&store);
    let mut client = sql(&url, &ns);
    client
        .execute(
            "DELETE FROM execution_context_checkpoints WHERE run_id='run'",
            &[],
        )
        .unwrap();
    let before: (String, i64) = {
        let row = client
            .query_one(
                "SELECT status,last_sequence FROM execution_runs WHERE id='run'",
                &[],
            )
            .unwrap();
        (row.get(0), row.get(1))
    };
    store.migrate().unwrap();
    assert!(matches!(
        store.run("owner", "session", "run"),
        Err(StoreError::MigrationRequired)
    ));
    let row = client
        .query_one(
            "SELECT status,last_sequence FROM execution_runs WHERE id='run'",
            &[],
        )
        .unwrap();
    assert_eq!((row.get::<_, String>(0), row.get::<_, i64>(1)), before);
}

#[test]
fn idle_stores_do_not_reserve_their_entire_connection_capacity() {
    let (_store, url, ns) = database();
    // More than the server's 100 default connections if every eight-slot pool
    // eagerly opens all slots. Keep stores alive simultaneously to test this.
    let stores: Vec<_> = (0..16)
        .map(|_| PostgresStore::connect(&url, &ns).unwrap())
        .collect();
    for store in stores {
        store.create_session("owner", "shared-session").unwrap();
    }
}

#[test]
fn trace_and_chat_events_share_the_same_progress_payload() {
    let (store, _, _) = database();
    running(&store);
    let payload = json!({"type":"tool_progress","tool_call_id":"render-1","progress":42.5,"total":100,"message":"真实服务进度"});
    store
        .chat_event("owner", "session", "run", payload.clone())
        .unwrap();
    let envelopes = store.events("owner", "session", "run", 0, 100).unwrap();
    assert!(
        envelopes.iter().any(
            |event| matches!(&event.event, ExecutionEvent::Chat { payload: p } if p == &payload)
        )
    );
    let trace = store.trace_run("run", 0, 100).unwrap();
    let progress = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["event_type"] == "tool_progress")
        .expect("Trace must decode data.payload, not return null");
    assert_eq!(progress["payload"], payload);
    assert!(
        trace["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["event_type"] == "pro_run_created")
    );
}

#[test]
fn migration_locks_are_namespace_scoped_and_same_namespace_remains_serialized() {
    let (first, url, namespace) = database();
    let other_namespace = format!("{namespace}_other");
    let other = PostgresStore::connect(&url, &other_namespace).unwrap();
    let mut blocker = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    let mut guard = blocker.transaction().unwrap();
    // A legacy/global lock from an independent installation must not stop
    // initialization of this test's separate namespace.
    guard
        .query_one("SELECT pg_advisory_xact_lock(4271072001)", &[])
        .unwrap();
    guard
        .query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072001))",
            &[&namespace],
        )
        .unwrap();
    other.migrate().unwrap();
    // The first namespace is genuinely locked: its competing migration must
    // respect the existing lock_timeout and leave the transaction rolled back.
    assert!(matches!(first.migrate(), Err(StoreError::Database)));
    guard.rollback().unwrap();
    first.migrate().unwrap();
    other.health().unwrap();
}
