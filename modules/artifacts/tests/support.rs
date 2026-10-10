use exoharness_artifacts::Artifacts;
use exoharness_contracts::execution::{ExecutionSnapshot, ModelSnapshot};
use exoharness_storage_postgres::{CreateTurn, PostgresStore};
use serde_json::json;
use std::{
    fs::File,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};
pub fn fixture() -> (tempfile::TempDir, PostgresStore, Artifacts) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let store = PostgresStore::connect(
        &std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated DB"),
        &format!(
            "artifact_test_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ),
    )
    .unwrap();
    store.migrate().unwrap();
    store.create_session("owner", "session").unwrap();
    let snapshot = ExecutionSnapshot {
        version: 1,
        assistant_id: "assistant".into(),
        assistant_revision: 1,
        model: ModelSnapshot {
            config_id: "test".into(),
            revision: 1,
            adapter: "fixture".into(),
            settings: json!({}),
        },
        capabilities: vec![],
        max_consecutive_identical_calls: 3,
        requires_execution: false,
    };
    store
        .create_turn(CreateTurn {
            owner: "owner",
            session_id: "session",
            expected_session_version: 0,
            turn_id: "turn",
            run_id: "run",
            input: "fixture",
            snapshot: &snapshot,
        })
        .unwrap();
    let artifacts = Artifacts::open(
        root.path().join("files"),
        store.clone(),
        "http://127.0.0.1:9790",
    )
    .unwrap();
    (root, store, artifacts)
}
pub fn file(bytes: &[u8]) -> File {
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(bytes).unwrap();
    file
}
pub fn access(url: &str) -> (u64, String) {
    let url = url::Url::parse(url).unwrap();
    let query = url
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    (
        query["expires"].parse().unwrap(),
        query["signature"].to_string(),
    )
}
