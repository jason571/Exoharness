//! Real PostgreSQL accounting across multiple model calls, not a mocked sum.
use exoharness_storage_postgres::*;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

fn fixture() -> (PostgresStore, postgres::Client) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("disposable PostgreSQL required");
    let ns = format!(
        "usage_test_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = PostgresStore::connect(&url, &ns).unwrap();
    store.migrate().unwrap();
    store.create_session("owner", "session").unwrap();
    let snapshot = ExecutionSnapshot {
        version: 1,
        assistant_id: "assistant".into(),
        assistant_revision: 1,
        model: ModelSnapshot {
            config_id: "model".into(),
            revision: 1,
            adapter: "mock".into(),
            settings: json!({}),
        },
        capabilities: vec![],
        max_consecutive_identical_calls: 3,
        requires_execution: false,
    };
    let run = store
        .create_turn(CreateTurn {
            owner: "owner",
            session_id: "session",
            expected_session_version: 0,
            turn_id: "turn",
            run_id: "run",
            input: "generate",
            snapshot: &snapshot,
        })
        .unwrap();
    store
        .transition_run(
            "owner",
            "session",
            "run",
            run.version,
            RunState::Running,
            None,
        )
        .unwrap();
    let mut client = postgres::Client::connect(&url, postgres::NoTls).unwrap();
    client
        .batch_execute(&format!("SET search_path={ns},public"))
        .unwrap();
    (store, client)
}
fn budget(client: &mut postgres::Client) -> Value {
    let row = client
        .query_one(
            "SELECT usage_json FROM execution_run_budgets WHERE run_id='run'",
            &[],
        )
        .unwrap();
    serde_json::from_str(&row.get::<_, String>(0)).unwrap()
}
fn response(input: u64, output: u64, cache: u64) -> Value {
    json!({"usage":{"input_tokens":input,"output_tokens":output,"cache_read_tokens":cache,"cache_write_tokens":0,"total_tokens":input+output}})
}

#[test]
fn committed_calls_accumulate_without_double_counting_cache_or_duplicate_completion() {
    let (store, mut client) = fixture();
    for seq in 1..=3 {
        store
            .chat_model_start_sequence("owner", "session", "run", seq, &json!({}))
            .unwrap();
    }
    store
        .chat_model_finish_sequence(
            "owner",
            "session",
            "run",
            1,
            &response(1000, 100, 800),
            None,
        )
        .unwrap();
    assert_eq!(budget(&mut client)["total_tokens"], 1100);
    store
        .chat_model_finish_sequence(
            "owner",
            "session",
            "run",
            2,
            &response(2000, 200, 1500),
            None,
        )
        .unwrap();
    assert_eq!(budget(&mut client)["total_tokens"], 3300); // Third request is still submitted.
    assert!(matches!(
        store.chat_model_finish_sequence(
            "owner",
            "session",
            "run",
            2,
            &response(2000, 200, 1500),
            None
        ),
        Err(StoreError::Conflict)
    ));
    assert_eq!(budget(&mut client)["total_tokens"], 3300);
    store
        .chat_model_finish_sequence(
            "owner",
            "session",
            "run",
            3,
            &response(3000, 300, 2000),
            Some("provider error"),
        )
        .unwrap();
    let usage = budget(&mut client);
    assert_eq!(usage["input_tokens"], 6000);
    assert_eq!(usage["output_tokens"], 600);
    assert_eq!(usage["cache_read_tokens"], 4300);
    assert_eq!(usage["total_tokens"], 6600);
    assert!(
        store
            .chat_model_finish_sequence(
                "other-owner",
                "session",
                "run",
                3,
                &response(1, 1, 1),
                None
            )
            .is_err()
    );
    assert_eq!(budget(&mut client), usage);
}

#[test]
fn unreported_usage_remains_unknown_instead_of_replaced_by_later_reported_call() {
    let (store, mut client) = fixture();
    store
        .chat_model_start_sequence("owner", "session", "run", 1, &json!({}))
        .unwrap();
    store
        .chat_model_finish_sequence(
            "owner",
            "session",
            "run",
            1,
            &json!({"usage":{"input_tokens":100,"cache_read_tokens":50}}),
            Some("truncated response"),
        )
        .unwrap();
    store
        .chat_model_start_sequence("owner", "session", "run", 2, &json!({}))
        .unwrap();
    store
        .chat_model_finish_sequence("owner", "session", "run", 2, &response(200, 20, 100), None)
        .unwrap();
    let usage = budget(&mut client);
    assert_eq!(usage["input_tokens"], 300);
    assert_eq!(usage["output_tokens"], Value::Null);
    assert_eq!(usage["total_tokens"], Value::Null);
    assert_eq!(usage["cache_read_tokens"], 150);
    assert_eq!(usage["cache_write_tokens"], Value::Null);
}
