use exoharness_secrets::SecretCipher;

#[test]
fn generated_application_keys_use_exo_prefix_and_256_bit_random_payload() {
    let first = SecretCipher::generate_key().unwrap();
    let second = SecretCipher::generate_key().unwrap();
    for key in [&first, &second] {
        let random = key.strip_prefix("exo_").unwrap();
        assert_eq!(random.len(), 64);
        assert!(
            random
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        );
    }
    assert_ne!(first, second);
}

#[test]
fn encrypted_key_survives_restart_and_rejects_another_application_or_tampering() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = root.path().join("keys");
    let cipher = SecretCipher::open(&path).unwrap();
    let key = SecretCipher::generate_key().unwrap();
    let first = cipher.seal("application:a", &key).unwrap();
    let second = cipher.seal("application:a", &key).unwrap();
    assert_ne!(first, second);
    assert!(!first.contains(&key));
    assert_eq!(
        SecretCipher::open(&path)
            .unwrap()
            .open_secret("application:a", &first)
            .unwrap(),
        key
    );
    assert!(cipher.open_secret("application:b", &first).is_err());
    let mut bad = first.clone();
    bad.pop();
    bad.push(if first.ends_with('0') { '1' } else { '0' });
    assert!(cipher.open_secret("application:a", &bad).is_err());
    assert!(cipher.open_secret("application:a", "v1.not-hex").is_err());
    let legacy = format!("exs_{}", "ab".repeat(32));
    let legacy_encrypted = cipher.seal("application:legacy", &legacy).unwrap();
    assert_eq!(
        SecretCipher::open(&path)
            .unwrap()
            .open_secret("application:legacy", &legacy_encrypted)
            .unwrap(),
        legacy
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(path.join("master-key"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
}
#[test]
fn concurrent_master_key_creation_uses_one_durable_key() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let path = root.path().join("keys");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let jobs = (0..4)
        .map(|_| {
            let path = path.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                SecretCipher::open(&path)
                    .unwrap()
                    .seal("same-app", "test-secret")
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let reader = SecretCipher::open(&path).unwrap();
    for job in jobs {
        assert_eq!(
            reader
                .open_secret("same-app", &job.join().unwrap())
                .unwrap(),
            "test-secret"
        );
    }
}
