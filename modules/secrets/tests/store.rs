use exoharness_secrets::{SecretError, SecretStore};
fn fixture() -> (tempfile::TempDir, SecretStore) {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let store = SecretStore::open(root.path().into()).unwrap();
    (root, store)
}
#[test]
fn full_header_is_preserved_rotation_is_durable_and_debug_is_redacted() {
    let (root, store) = fixture();
    let key = store.put("Bearer fixture-only").unwrap();
    assert_eq!(store.get(&key).unwrap().expose(), "Bearer fixture-only");
    assert!(!format!("{:?}", store.get(&key).unwrap()).contains("fixture-only"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(root.path().join(format!("{key}.secret")))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    store.rotate(&key, "Basic fixture-new").unwrap();
    assert_eq!(
        SecretStore::open(root.path().into())
            .unwrap()
            .get(&key)
            .unwrap()
            .expose(),
        "Basic fixture-new"
    );
    store.delete(&key).unwrap();
    store.delete(&key).unwrap();
    assert!(matches!(store.get(&key), Err(SecretError::Missing)));
}
#[test]
fn unsafe_paths_symlinks_broad_permissions_and_header_injection_fail_closed() {
    let (root, store) = fixture();
    for value in ["", "a\nb", "a\rb", "x\0y"] {
        assert!(store.put(value).is_err());
    }
    for key in ["../secrets", "/etc/passwd", &"x".repeat(64)] {
        assert!(store.get(key).is_err());
        assert!(store.delete(key).is_err());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let key = store.put("safe").unwrap();
        let file = root.path().join(format!("{key}.secret"));
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(store.get(&key).is_err());
        let link = root.path().join(format!("{}.secret", "a".repeat(64)));
        symlink(file, &link).unwrap();
        assert!(store.get(&"a".repeat(64)).is_err());
        assert!(store.delete(&"a".repeat(64)).is_err());
    }
}
