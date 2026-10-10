mod support;
use exoharness_artifacts::Artifacts;
use std::io::Read;
use support::{access, file, fixture};
#[test]
fn permanent_signed_urls_survive_reopening_and_all_media_share_the_same_base() {
    let (root, store, artifacts) = fixture();
    for (name, mime) in [
        ("result.mp4", "video/mp4"),
        ("封面.png", "image/png"),
        ("歌曲.mp3", "audio/mpeg"),
        (
            "report.docx",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
    ] {
        let published = artifacts
            .publish_file("owner", "session", "run", name, file(b"fixture bytes"))
            .unwrap();
        assert!(
            published
                .url
                .starts_with("http://127.0.0.1:9790/artifacts/")
        );
        let (expires, signature) = access(&published.url);
        assert_eq!(expires, 0);
        let reopened = Artifacts::open(
            root.path().join("files"),
            store.clone(),
            "http://127.0.0.1:9790",
        )
        .unwrap();
        let (record, mut reader) = reopened
            .open_signed(&published.artifact.id, name, expires, &signature)
            .unwrap();
        assert_eq!(record.content_type, mime);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"fixture bytes");
        assert!(published.download_url.ends_with("&download=true"));
    }
}
#[test]
fn authorization_precedes_copy_and_file_name_or_signature_changes_cannot_reuse_access() {
    let (root, _, artifacts) = fixture();
    assert!(
        artifacts
            .publish_file("other", "session", "run", "secret.mp4", file(b"private"))
            .is_err()
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("files/objects"))
            .unwrap()
            .count(),
        0
    );
    for name in [
        "../x.mp4",
        "/etc/passwd",
        "script\n.mp4",
        "a/b.png",
        "a\\b.mp3",
    ] {
        assert!(
            artifacts
                .publish_file("owner", "session", "run", name, file(b"private"))
                .is_err()
        );
    }
    let published = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"private"))
        .unwrap();
    let (expires, signature) = access(&published.url);
    assert!(
        artifacts
            .open_signed(&published.artifact.id, "other.mp4", expires, &signature)
            .is_err()
    );
    assert!(
        artifacts
            .open_signed(
                &published.artifact.id,
                "result.mp4",
                expires,
                &"😀".repeat(16)
            )
            .is_err()
    );
    assert!(
        artifacts
            .open_signed(
                &published.artifact.id,
                "result.mp4",
                expires,
                &"0".repeat(64)
            )
            .is_err()
    );
    let expired = artifacts
        .url(&published.artifact.id, "result.mp4", 1, false)
        .unwrap();
    let (expires, signature) = access(&expired);
    assert!(
        artifacts
            .open_signed(&published.artifact.id, "result.mp4", expires, &signature)
            .is_err()
    );
}
#[test]
fn repeated_publish_is_idempotent_and_objects_never_overwrite_existing_bytes() {
    let (_root, _, artifacts) = fixture();
    let first = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"same"))
        .unwrap();
    let second = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"same"))
        .unwrap();
    assert_eq!(first.artifact, second.artifact);
    assert_eq!(first.url, second.url);
    let changed = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"changed"))
        .unwrap();
    assert_ne!(first.artifact.id, changed.artifact.id);
    let (expires, signature) = access(&first.url);
    let (_, mut reader) = artifacts
        .open_signed(&first.artifact.id, "result.mp4", expires, &signature)
        .unwrap();
    let mut bytes = String::new();
    reader.read_to_string(&mut bytes).unwrap();
    assert_eq!(bytes, "same");
}
#[test]
fn delivery_rejects_symlinks_changed_sizes_and_unsafe_public_bases() {
    let (root, store, artifacts) = fixture();
    for base in [
        "file:///tmp",
        "https://user:password@example.test",
        "https://example.test?secret=x",
    ] {
        assert!(Artifacts::open(root.path().join("other"), store.clone(), base).is_err());
    }
    let published = artifacts
        .publish_file("owner", "session", "run", "result.mp4", file(b"12345"))
        .unwrap();
    let path = root
        .path()
        .join("files")
        .join(&published.artifact.relative_path);
    std::fs::remove_file(&path).unwrap();
    std::os::unix::fs::symlink(root.path().join("files/delivery-key"), &path).unwrap();
    let (expires, signature) = access(&published.url);
    assert!(
        artifacts
            .open_signed(&published.artifact.id, "result.mp4", expires, &signature)
            .is_err()
    );
}
