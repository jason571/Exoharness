use super::*;
use exoharness_sandbox::{TaskEdit, TaskPatch};
#[tokio::test]
async fn failure_preserves_files_local_patch_reuses_them_and_restart_keeps_receipts() {
    let (root, sandbox) = fixture().await;
    let store = sandbox.task_store().unwrap();
    let saved = store.create("alice", "session", "task", None).unwrap();
    let saved=store.edit("alice","session",&saved.workspace_id,1,"source",TaskEdit{files:BTreeMap::from([("main.py".into(),"from pathlib import Path\nPath('retained.txt').write_text('do not regenerate')\nraise RuntimeError('fix only this step')".into())]),..Default::default()}).unwrap();
    let active = store
        .begin("alice", "session", &saved.workspace_id, 2, "first-job")
        .unwrap();
    assert!(matches!(
        store.edit(
            "alice",
            "session",
            &saved.workspace_id,
            2,
            "concurrent",
            TaskEdit::default()
        ),
        Err(SandboxError::WorkspaceBusy)
    ));
    let receipt = sandbox
        .start(RunSpec {
            invocation_id: "first-job".into(),
            workspace: active.workspace.clone(),
            read_only_package: None,
            runtime: Runtime::Python,
            entrypoint: "main.py".into(),
            arguments: vec![],
            limits: Limits::default(),
        })
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(receipt.status, "failed");
    let saved = active.finish(&receipt).unwrap();
    assert_eq!(saved.revision, 3);
    assert_eq!(
        store
            .read(
                "alice",
                "session",
                &saved.workspace_id,
                "retained.txt",
                0,
                100,
                None
            )
            .unwrap()["content"],
        "do not regenerate"
    );
    let hash = saved
        .files
        .iter()
        .find(|f| f.path == "main.py")
        .unwrap()
        .sha256
        .clone();
    let fixed = store
        .edit(
            "alice",
            "session",
            &saved.workspace_id,
            3,
            "repair",
            TaskEdit {
                patches: vec![TaskPatch {
                    path: "main.py".into(),
                    sha256: hash,
                    before: "raise RuntimeError('fix only this step')".into(),
                    after: "print('repaired')".into(),
                }],
                ..Default::default()
            },
        )
        .unwrap();
    let active = store
        .begin(
            "alice",
            "session",
            &saved.workspace_id,
            fixed.revision,
            "second-job",
        )
        .unwrap();
    let receipt = sandbox
        .start(RunSpec {
            invocation_id: "second-job".into(),
            workspace: active.workspace.clone(),
            read_only_package: None,
            runtime: Runtime::Python,
            entrypoint: "main.py".into(),
            arguments: vec![],
            limits: Limits::default(),
        })
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(receipt.exit_code, Some(0));
    let saved = active.finish(&receipt).unwrap();
    assert!(matches!(
        store.begin(
            "alice",
            "session",
            &saved.workspace_id,
            saved.revision,
            "second-job"
        ),
        Err(SandboxError::OutcomeUnknown)
    ));
    let image = sandbox.image_id().to_owned();
    sandbox.stop().await.unwrap();
    let reopened = Sandbox::open(root.path().into(), &image).await.unwrap();
    let recovered = reopened
        .task_store()
        .unwrap()
        .inspect("alice", "session", &saved.workspace_id)
        .unwrap();
    assert_eq!(recovered.revision, saved.revision);
    assert!(
        reopened
            .task_store()
            .unwrap()
            .job("bob", "session", &saved.workspace_id, "second-job")
            .is_err()
    );
    reopened.stop().await.unwrap();
}
#[tokio::test]
async fn scoped_cancel_confirms_cleanup_and_saves_files_before_allowing_another_execution() {
    let (_root, sandbox) = fixture().await;
    let store = sandbox.task_store().unwrap();
    let saved = store.create("alice", "s", "task", None).unwrap();
    let saved = store
        .edit(
            "alice",
            "s",
            &saved.workspace_id,
            1,
            "source",
            TaskEdit {
                files: BTreeMap::from([(
                    "main.sh".into(),
                    "printf ready > retained.txt\nsleep 30".into(),
                )]),
                ..Default::default()
            },
        )
        .unwrap();
    let active = store
        .begin(
            "alice",
            "s",
            &saved.workspace_id,
            saved.revision,
            "cancel-job",
        )
        .unwrap();
    let job = sandbox
        .start(RunSpec {
            invocation_id: "cancel-job".into(),
            workspace: active.workspace.clone(),
            read_only_package: None,
            runtime: Runtime::Shell,
            entrypoint: "main.sh".into(),
            arguments: vec![],
            limits: Limits::default(),
        })
        .unwrap();
    for _ in 0..100 {
        if active.workspace.path().join("retained.txt").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(active.workspace.path().join("retained.txt").exists());
    assert!(sandbox.cancel_invocation("cancel-job").unwrap());
    let receipt = job.finish().await.unwrap();
    assert_eq!(receipt.status, "cancelled");
    assert!(receipt.cleanup_confirmed);
    let saved = active.finish(&receipt).unwrap();
    assert!(saved.running_invocation.is_none());
    assert_eq!(
        store
            .read(
                "alice",
                "s",
                &saved.workspace_id,
                "retained.txt",
                0,
                100,
                None
            )
            .unwrap()["content"],
        "ready"
    );
    sandbox.stop().await.unwrap();
}
