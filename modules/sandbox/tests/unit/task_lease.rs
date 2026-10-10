use super::*;
#[test]
fn task_lease_releases_inherited_description_without_releasing_another_owners_lock() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let store = TaskStore::open(root.path().into()).unwrap();
    let task = store.create("alice", "session", "create", None).unwrap();
    let directory = store.directory(&task.workspace_id).unwrap();
    let lease = store.lock(&directory).unwrap();
    let inherited = lease.0.try_clone().unwrap();
    assert!(matches!(
        store.lock(&directory),
        Err(SandboxError::WorkspaceBusy)
    ));
    drop(lease);
    let next = store.lock(&directory).unwrap();
    drop(inherited);
    assert!(matches!(
        store.lock(&directory),
        Err(SandboxError::WorkspaceBusy)
    ));
    drop(next);
    assert!(store.lock(&directory).is_ok());
}
