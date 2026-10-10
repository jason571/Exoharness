use exoharness_sandbox::{SandboxError, TaskEdit, TaskStore};
use std::collections::BTreeMap;
fn fixture() -> (tempfile::TempDir, TaskStore) {
    let root = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let store = TaskStore::open(root.path().into()).unwrap();
    (root, store)
}
#[test]
fn patch_is_atomic_scoped_versioned_and_idempotent_after_restart() {
    let (root, store) = fixture();
    let initial = store.create("a", "s", "create", None).unwrap();
    let saved = store
        .edit(
            "a",
            "s",
            &initial.workspace_id,
            1,
            "source",
            TaskEdit {
                files: BTreeMap::from([("index.html".into(), "old\n".into())]),
                ..Default::default()
            },
        )
        .unwrap();
    let patch = "*** Begin Patch\n*** Update File: index.html\n@@\n-old\n+new\n*** Add File: assets/app.js\n+console.log('ok');\n*** End Patch";
    assert!(
        store
            .apply_patch("b", "s", &saved.workspace_id, 2, "foreign", patch)
            .is_err()
    );
    let changed = store
        .apply_patch("a", "s", &saved.workspace_id, 2, "patch", patch)
        .unwrap();
    assert_eq!(changed.revision, 3);
    let restarted = TaskStore::open(root.path().into()).unwrap();
    assert_eq!(
        restarted
            .apply_patch("a", "s", &saved.workspace_id, 2, "patch", patch)
            .unwrap()
            .revision,
        3
    );
    assert!(matches!(
        store.apply_patch("a", "s", &saved.workspace_id, 2, "other", patch),
        Err(SandboxError::StaleRevision)
    ));
    let bad = "*** Begin Patch\n*** Add File: should-not-exist\n+bad\n*** Update File: index.html\n@@\n-missing\n+changed\n*** End Patch";
    assert!(
        store
            .apply_patch("a", "s", &saved.workspace_id, 3, "bad", bad)
            .is_err()
    );
    assert_eq!(
        store
            .inspect("a", "s", &saved.workspace_id)
            .unwrap()
            .files
            .len(),
        2
    );
    assert_eq!(
        store
            .read("a", "s", &saved.workspace_id, "index.html", 0, 100, None)
            .unwrap()["content"],
        "new\n"
    );
}
#[test]
fn search_cursor_subtree_binary_and_large_file_diagnostics_are_explicit() {
    let (_root, store) = fixture();
    let initial = store.create("a", "s", "create", None).unwrap();
    let saved = store
        .edit(
            "a",
            "s",
            &initial.workspace_id,
            1,
            "source",
            TaskEdit {
                files: BTreeMap::from([
                    ("src/a".into(), "查找1\n查找2\n".into()),
                    ("src/b".into(), "查找3\n".into()),
                    ("src/binary".into(), "\0查找".into()),
                    ("src/large".into(), "字".repeat(400000)),
                    ("other".into(), "查找其他".into()),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(
        store
            .search("b", "s", &saved.workspace_id, "查找", "", 0, 0, 20)
            .is_err()
    );
    let first = store
        .search("a", "s", &saved.workspace_id, "查找", "src", 0, 0, 1)
        .unwrap();
    assert_eq!(first["matches"][0]["text"], "查找1");
    let next = &first["next_cursor"];
    let second = store
        .search(
            "a",
            "s",
            &saved.workspace_id,
            "查找",
            "src",
            next["file_offset"].as_u64().unwrap() as usize,
            next["line_offset"].as_u64().unwrap() as usize,
            20,
        )
        .unwrap();
    assert_eq!(second["matches"].as_array().unwrap().len(), 2);
    assert_eq!(second["skipped_files"].as_array().unwrap().len(), 2);
    assert_eq!(second["complete"], true);
    assert!(second["next_cursor"].is_null());
}
