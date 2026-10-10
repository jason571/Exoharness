//! Actual cached Linux OCI execution, never mocked or silently skipped.
use exoharness_sandbox::{Limits, RunSpec, Runtime, Sandbox, SandboxError, Workspace};
use std::{collections::BTreeMap, path::Path, time::Duration};
#[path = "unit/task_execution.rs"]
mod task_execution;
async fn fixture() -> (tempfile::TempDir, Sandbox) {
    let image = std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE")
        .expect("set an already prepared local test image; no automatic pull/build");
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let sandbox = Sandbox::open(root.path().into(), &image).await.unwrap();
    (root, sandbox)
}

#[tokio::test]
async fn provider_lease_prevents_parallel_owners_and_reconciles_only_reserved_orphans() {
    use sha2::{Digest, Sha256};
    let (root, sandbox) = fixture().await;
    let image = sandbox.image_id().to_owned();
    assert!(matches!(
        Sandbox::open(root.path().into(), &image).await,
        Err(SandboxError::Unavailable)
    ));
    sandbox.stop().await.unwrap();
    let owner = format!(
        "{:x}",
        Sha256::digest(root.path().to_string_lossy().as_bytes())
    );
    let id = format!("{:x}", Sha256::digest(b"orphan-intent"));
    let reservation = root.path().join("jobs").join(&id);
    std::fs::write(&reservation, b"reserved; reconcile before resubmission\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&reservation, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let name = format!("exoharness-pro-job-{}-{id}", &owner[..16]);
    let label = format!("exoharness_owner={owner}");
    let created = tokio::process::Command::new("docker")
        .args([
            "run",
            "--detach",
            "--pull=never",
            "--network",
            "none",
            "--read-only",
            "--name",
            &name,
            "--label",
            &label,
            &image,
            "/bin/bash",
            "-c",
            "sleep 30",
        ])
        .output()
        .await
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let recovered = Sandbox::open(root.path().into(), &image).await.unwrap();
    let inspect = tokio::process::Command::new("docker")
        .args(["inspect", &name])
        .output()
        .await
        .unwrap();
    assert!(!inspect.status.success());
    assert!(reservation.exists());
    assert!(recovered.receipt("orphan-intent").unwrap().is_none());
    let work = workspace(
        &recovered,
        "after-orphan",
        "main.py",
        "print('must not repeat')",
    );
    assert!(matches!(
        recovered.start(spec("orphan-intent", work, Runtime::Python, "main.py")),
        Err(SandboxError::OutcomeUnknown)
    ));
    recovered.stop().await.unwrap();
}

#[tokio::test]
async fn cpu_memory_and_pids_limits_are_visible_inside_the_real_cgroup() {
    let (_root, sandbox) = fixture().await;
    let source = "from pathlib import Path\np=Path('/sys/fs/cgroup')\nassert int((p/'memory.max').read_text())==268435456\nassert int((p/'pids.max').read_text())==32\nquota,period=(p/'cpu.max').read_text().split()\nassert int(quota)*2==int(period)\nprint('resource-limits-enforced')";
    let work = workspace(&sandbox, "cgroups", "main.py", source);
    let mut job = spec("cgroups", work, Runtime::Python, "main.py");
    job.limits.memory_bytes = 256 * 1024 * 1024;
    job.limits.cpu_millis = 500;
    job.limits.pids = 32;
    let result = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(result.status, "completed", "{}", result.stderr);
    assert_eq!(result.stdout.trim(), "resource-limits-enforced");
    sandbox.stop().await.unwrap();
}

#[tokio::test]
async fn memory_overrun_is_killed_without_losing_preexisting_files() {
    let (_root, sandbox) = fixture().await;
    let work = workspace(
        &sandbox,
        "oom",
        "main.py",
        "from pathlib import Path\nPath('evidence.txt').write_text('before-oom')\nvalue=bytearray(256*1024*1024)\nprint('unexpected')",
    );
    let path = work.path().to_owned();
    let mut job = spec("oom", work, Runtime::Python, "main.py");
    job.limits.memory_bytes = 64 * 1024 * 1024;
    let result = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(result.exit_code, Some(137));
    assert!(result.cleanup_confirmed);
    assert_eq!(
        std::fs::read_to_string(path.join("evidence.txt")).unwrap(),
        "before-oom"
    );
    assert!(sandbox.receipt("oom").unwrap().unwrap().cleanup_confirmed);
    sandbox.stop().await.unwrap();
}
fn workspace(sandbox: &Sandbox, name: &str, file: &str, source: &str) -> Workspace {
    sandbox
        .create_workspace(
            name,
            BTreeMap::from([(file.into(), source.as_bytes().into())]),
        )
        .unwrap()
}
fn spec(id: &str, workspace: Workspace, runtime: Runtime, entrypoint: &str) -> RunSpec {
    RunSpec {
        invocation_id: id.into(),
        workspace,
        read_only_package: None,
        runtime,
        entrypoint: entrypoint.into(),
        arguments: vec![],
        limits: Limits::default(),
    }
}
async fn file_exists(path: &Path) {
    tokio::time::timeout(Duration::from_secs(8), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn all_four_runtimes_execute_inside_cached_nonroot_pinned_image() {
    let (_root, sandbox) = fixture().await;
    for (id, runtime, file, source, expected) in [
        (
            "python",
            Runtime::Python,
            "main.py",
            "from pathlib import Path\nimport os\nassert os.getuid()!=0\nPath('result.txt').write_text('durable')\nprint(7*6)",
            "42",
        ),
        (
            "shell",
            Runtime::Shell,
            "main.sh",
            "printf shell-ok",
            "shell-ok",
        ),
        (
            "js",
            Runtime::Javascript,
            "main.mjs",
            "console.log('javascript-ok')",
            "javascript-ok",
        ),
        (
            "ts",
            Runtime::Typescript,
            "main.ts",
            "enum Kind { A='typescript-ok' }; const value: string = Kind.A; console.log(value)",
            "typescript-ok",
        ),
    ] {
        let workspace = workspace(&sandbox, id, file, source);
        let path = workspace.path().to_owned();
        let result = sandbox
            .start(spec(id, workspace, runtime, file))
            .unwrap()
            .finish()
            .await
            .unwrap();
        assert_eq!(result.status, "completed", "{}", result.stderr);
        assert_eq!(result.stdout.trim(), expected);
        assert!(result.image_id.starts_with("sha256:"));
        assert_eq!(result.exit_code, Some(0));
        if runtime == Runtime::Python {
            assert_eq!(
                std::fs::read_to_string(path.join("result.txt")).unwrap(),
                "durable"
            );
        }
    }
    sandbox.stop().await.unwrap();
}
#[tokio::test]
async fn roots_readonly_package_and_network_are_actually_confined() {
    let (_root, sandbox) = fixture().await;
    let package = workspace(&sandbox, "package", "resource.txt", "original");
    let package_path = package.path().to_owned();
    let source = "import pathlib,socket,os\nassert os.getuid()!=0\nassert not pathlib.Path('/mnt/e/workspace/exoharness').exists()\nfor p in ['/package/resource.txt','/etc/sandbox-escape']:\n try: pathlib.Path(p).write_text('bad')\n except OSError: pass\n else: raise AssertionError('unexpected writable mount')\ns=socket.socket();s.settimeout(.2)\ntry: s.connect(('1.1.1.1',80))\nexcept OSError: print('confined')\nelse: raise AssertionError('unexpected network')\n";
    let workspace = workspace(&sandbox, "confined", "main.py", source);
    let mut job = spec("confined", workspace, Runtime::Python, "main.py");
    job.read_only_package = Some(package);
    let result = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(result.status, "completed", "{}", result.stderr);
    assert_eq!(result.stdout.trim(), "confined");
    assert_eq!(
        std::fs::read_to_string(package_path.join("resource.txt")).unwrap(),
        "original"
    );
    sandbox.stop().await.unwrap();
}

#[tokio::test]
async fn interrupted_stop_keeps_resources_owned_and_workspace_cannot_overlap() {
    let (_root, sandbox) = fixture().await;
    let workspace = workspace(
        &sandbox,
        "ownership",
        "main.sh",
        "printf started > started; sleep 30; printf bad > should-not-exist",
    );
    let path = workspace.path().to_owned();
    let handle = sandbox
        .start(spec("owned", workspace.clone(), Runtime::Shell, "main.sh"))
        .unwrap();
    file_exists(&path.join("started")).await;
    assert!(matches!(
        sandbox.start(spec(
            "overlap",
            workspace.clone(),
            Runtime::Shell,
            "main.sh"
        )),
        Err(SandboxError::WorkspaceBusy)
    ));
    let _ = tokio::time::timeout(Duration::from_millis(1), sandbox.stop()).await;
    sandbox.stop().await.unwrap();
    assert_eq!(handle.finish().await.unwrap().status, "cancelled");
    assert!(!path.join("should-not-exist").exists());
    assert!(matches!(
        sandbox.start(spec("after-stop", workspace, Runtime::Shell, "main.sh")),
        Err(SandboxError::Unavailable)
    ));
}

#[tokio::test]
async fn invalid_utf8_cannot_expand_output_past_the_declared_byte_budget() {
    let (_root, sandbox) = fixture().await;
    let workspace = workspace(
        &sandbox,
        "utf8",
        "main.py",
        "import os\nos.write(1,b'\\xff'*2000)",
    );
    let mut job = spec("utf8", workspace, Runtime::Python, "main.py");
    job.limits.output_bytes = 1024;
    let receipt = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(receipt.status, "completed");
    assert!(receipt.output_truncated);
    assert!(receipt.stdout.len() + receipt.stderr.len() <= 1024);
    sandbox.stop().await.unwrap();
}
#[tokio::test]
async fn explicit_cancel_timeout_and_dropped_consumer_stop_the_entire_job() {
    let (_root, sandbox) = fixture().await;
    for id in ["explicit", "drop"] {
        let workspace = workspace(
            &sandbox,
            id,
            "main.sh",
            "printf started > started; sleep 30; printf bad > should-not-exist",
        );
        let path = workspace.path().to_owned();
        let handle = sandbox
            .start(spec(id, workspace, Runtime::Shell, "main.sh"))
            .unwrap();
        file_exists(&path.join("started")).await;
        if id == "explicit" {
            handle.cancel();
            assert_eq!(handle.finish().await.unwrap().status, "cancelled");
        } else {
            drop(handle);
        }
        assert!(!path.join("should-not-exist").exists());
    }
    let workspace = workspace(
        &sandbox,
        "timeout",
        "main.py",
        "import time\ntime.sleep(30)",
    );
    let mut job = spec("timeout", workspace, Runtime::Python, "main.py");
    job.limits.timeout = Duration::from_millis(100);
    let result = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(result.status, "timed_out");
    sandbox.stop().await.unwrap();
}
#[tokio::test]
async fn repeated_invocation_survives_provider_restart_and_saved_files_are_unchanged() {
    let (root, sandbox) = fixture().await;
    let workspace = workspace(&sandbox, "repeat", "main.py", "print('once')");
    let result = sandbox
        .start(spec(
            "stable-invocation",
            workspace.clone(),
            Runtime::Python,
            "main.py",
        ))
        .unwrap()
        .finish()
        .await
        .unwrap();
    assert_eq!(result.status, "completed");
    assert!(matches!(
        sandbox.start(spec(
            "stable-invocation",
            workspace.clone(),
            Runtime::Python,
            "main.py"
        )),
        Err(SandboxError::OutcomeUnknown)
    ));
    let image = sandbox.image_id().to_owned();
    sandbox.stop().await.unwrap();
    let reopened = Sandbox::open(root.path().into(), &image).await.unwrap();
    let receipt = reopened.receipt("stable-invocation").unwrap().unwrap();
    assert_eq!(receipt.stdout.trim(), "once");
    assert!(receipt.cleanup_confirmed);
    assert!(matches!(
        reopened.start(spec(
            "stable-invocation",
            workspace.clone(),
            Runtime::Python,
            "main.py"
        )),
        Err(SandboxError::OutcomeUnknown)
    ));
    assert_eq!(
        std::fs::read_to_string(workspace.path().join("main.py")).unwrap(),
        "print('once')"
    );
    reopened.stop().await.unwrap();
}
#[tokio::test]
async fn bounded_output_and_nonzero_exit_are_preserved_not_model_summarized() {
    let (_root, sandbox) = fixture().await;
    let workspace = workspace(
        &sandbox,
        "output",
        "main.py",
        "print('x'*100000)\nraise SystemExit(7)",
    );
    let mut job = spec("bounded", workspace, Runtime::Python, "main.py");
    job.limits.output_bytes = 1024;
    let result = sandbox.start(job).unwrap().finish().await.unwrap();
    assert_eq!(result.status, "failed");
    assert_eq!(result.exit_code, Some(7));
    assert!(result.output_truncated);
    assert!(result.stdout.len() + result.stderr.len() <= 1024);
    sandbox.stop().await.unwrap();
}
#[tokio::test]
async fn invalid_paths_runtime_and_missing_image_never_fall_back_to_host() {
    let (root, sandbox) = fixture().await;
    for path in [
        "../outside.py",
        "/tmp/outside.py",
        "folder/../outside.py",
        "C:\\outside.py",
    ] {
        assert!(matches!(
            sandbox.create_workspace(
                path,
                BTreeMap::from([(path.into(), b"raise Exception()".to_vec())])
            ),
            Err(SandboxError::Invalid)
        ));
    }
    let workspace = workspace(&sandbox, "mismatch", "main.sh", "printf bad");
    assert!(matches!(
        sandbox.start(spec(
            "mismatch",
            workspace.clone(),
            Runtime::Python,
            "main.sh"
        )),
        Err(SandboxError::RuntimeMismatch)
    ));
    assert!(
        std::fs::read_dir(root.path().join("jobs"))
            .unwrap()
            .next()
            .is_none()
    );
    #[cfg(unix)]
    {
        std::fs::remove_file(workspace.path().join("main.sh")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", workspace.path().join("main.sh")).unwrap();
        assert!(matches!(
            sandbox.start(spec("symlink", workspace, Runtime::Shell, "main.sh")),
            Err(SandboxError::Invalid)
        ));
    }
    sandbox.stop().await.unwrap();
    assert!(matches!(
        Sandbox::open(
            root.path().into(),
            "exoharness-pro-do-not-pull:missing-test-image"
        )
        .await,
        Err(SandboxError::Unavailable)
    ));
}
