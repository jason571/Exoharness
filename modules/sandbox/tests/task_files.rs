use exoharness_sandbox::{SandboxError, TaskEdit, TaskPatch, TaskStore};
use std::collections::BTreeMap;
fn fixture() -> (tempfile::TempDir, TaskStore) {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let store = TaskStore::open(root.path().into()).unwrap();
    (root, store)
}
fn edit(files: &[(&str, &str)]) -> TaskEdit {
    TaskEdit {
        files: files
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        ..Default::default()
    }
}
#[test]
fn scope_persistence_exact_patch_and_atomic_conflict() {
    let (root, store) = fixture();
    let first = store.create("alice", "session-a", "create", None).unwrap();
    assert!(
        store
            .inspect("bob", "session-a", &first.workspace_id)
            .is_err()
    );
    assert!(
        store
            .inspect("alice", "session-b", &first.workspace_id)
            .is_err()
    );
    let saved = store
        .edit(
            "alice",
            "session-a",
            &first.workspace_id,
            1,
            "write",
            edit(&[
                ("main.py", "title = '中文'\nprint(title)\n"),
                ("data/input.json", "{\"year\":2025}"),
            ]),
        )
        .unwrap();
    assert_eq!(saved.revision, 2);
    let restarted = TaskStore::open(root.path().into()).unwrap();
    assert_eq!(
        restarted
            .inspect("alice", "session-a", &saved.workspace_id)
            .unwrap()
            .revision,
        2
    );
    let hash = saved
        .files
        .iter()
        .find(|f| f.path == "main.py")
        .unwrap()
        .sha256
        .clone();
    let patch = TaskEdit {
        patches: vec![TaskPatch {
            path: "main.py".into(),
            sha256: hash,
            before: "中文".into(),
            after: "正确中文".into(),
        }],
        ..Default::default()
    };
    let changed = store
        .edit(
            "alice",
            "session-a",
            &saved.workspace_id,
            2,
            "patch",
            patch.clone(),
        )
        .unwrap();
    assert_eq!(changed.revision, 3);
    // Same durable operation does not apply a patch again.
    assert_eq!(
        store
            .edit("alice", "session-a", &saved.workspace_id, 2, "patch", patch)
            .unwrap()
            .revision,
        3
    );
    assert!(matches!(
        store.edit(
            "alice",
            "session-a",
            &saved.workspace_id,
            2,
            "stale",
            edit(&[("other.py", "oops")])
        ),
        Err(SandboxError::StaleRevision)
    ));
    let bad = TaskEdit {
        files: BTreeMap::from([("should-not-exist.py".into(), "hello".into())]),
        deletes: vec!["missing.py".into()],
        ..Default::default()
    };
    assert!(
        store
            .edit(
                "alice",
                "session-a",
                &saved.workspace_id,
                3,
                "bad-batch",
                bad
            )
            .is_err()
    );
    assert_eq!(
        store
            .inspect("alice", "session-a", &saved.workspace_id)
            .unwrap()
            .files
            .len(),
        2
    );
    let read = store
        .read(
            "alice",
            "session-a",
            &saved.workspace_id,
            "main.py",
            0,
            16,
            Some("正确"),
        )
        .unwrap();
    assert!(read["content"].as_str().unwrap().starts_with("正确中文"));
    assert!(
        store
            .edit(
                "alice",
                "session-a",
                &saved.workspace_id,
                3,
                "escape",
                edit(&[("../escape.py", "oops")])
            )
            .is_err()
    );
    assert!(
        store
            .edit(
                "alice",
                "session-a",
                &saved.workspace_id,
                3,
                "immutable",
                edit(&[("skill/scripts/a.py", "oops")])
            )
            .is_err()
    );
}
#[test]
fn binary_and_ambiguous_anchors_do_not_reach_model_or_mutate() {
    let (_root, store) = fixture();
    let saved = store.create("alice", "s", "create", None).unwrap();
    let saved = store
        .edit(
            "alice",
            "s",
            &saved.workspace_id,
            1,
            "write",
            edit(&[("binary.txt", "\0binary"), ("same.py", "aaa")]),
        )
        .unwrap();
    assert!(
        store
            .read(
                "alice",
                "s",
                &saved.workspace_id,
                "binary.txt",
                0,
                8000,
                None
            )
            .is_err()
    );
    let hash = saved
        .files
        .iter()
        .find(|f| f.path == "same.py")
        .unwrap()
        .sha256
        .clone();
    let patch = TaskEdit {
        patches: vec![TaskPatch {
            path: "same.py".into(),
            sha256: hash,
            before: "aa".into(),
            after: "b".into(),
        }],
        ..Default::default()
    };
    assert!(matches!(
        store.edit("alice", "s", &saved.workspace_id, 2, "ambiguous", patch),
        Err(SandboxError::AmbiguousPatch)
    ));
    assert_eq!(
        store
            .inspect("alice", "s", &saved.workspace_id)
            .unwrap()
            .revision,
        2
    );
}
