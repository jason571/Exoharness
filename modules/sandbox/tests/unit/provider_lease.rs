use super::*;
#[tokio::test]
async fn drained_provider_explicitly_unlocks_even_when_file_description_was_inherited() {
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let image =
        std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE").expect("cached test image required");
    let sandbox = Sandbox::open(root.path().into(), &image).await.unwrap();
    let inherited = sandbox
        .inner
        .lease
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .try_clone()
        .unwrap();
    assert!(matches!(
        Sandbox::open(root.path().into(), &image).await,
        Err(SandboxError::Unavailable)
    ));
    sandbox.stop().await.unwrap();
    // The duplicate descriptor still exists, simulating fork before CLOEXEC.
    let reopened = Sandbox::open(root.path().into(), &image).await.unwrap();
    drop(inherited);
    reopened.stop().await.unwrap();
}
