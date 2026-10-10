mod support;
use exoharness_contracts::capability::PermissionDecision;
use exoharness_skill_runtime::{RuntimeError, SkillRuntime};
use exoharness_storage_postgres::RegistryAccess;
use support::{fixture, request};
const ADMIN: RegistryAccess = RegistryAccess::Administrator;
#[test]
fn disposed_runtime_rejects_stale_handles_without_spawning_any_probe() {
    let f = fixture();
    let record = f.enable(&f.import(&[("main.py", "print(1)")]));
    f.executor.block_on(f.runtime.close());
    assert!(matches!(
        f.executor
            .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision)),
        Err(RuntimeError::Unavailable)
    ));
    assert!(matches!(
        f.executor
            .block_on(f.runtime.status(ADMIN, record.id, record.revision)),
        Err(RuntimeError::Unavailable)
    ));
    assert_eq!(
        std::fs::read_dir(f._root.path().join("sandbox/jobs"))
            .unwrap()
            .count(),
        0
    );
    f.executor.block_on(f.sandbox.stop()).unwrap();
}

#[test]
fn uploaded_hooks_are_never_run_and_all_cached_interpreters_are_checked() {
    let f = fixture();
    let record = f.enable(&f.import(&[
        (
            "scripts/main.py",
            "raise RuntimeError('uploaded code must not run')",
        ),
        ("scripts/main.sh", "exit 99"),
        ("scripts/main.js", "throw new Error('not executed')"),
        ("scripts/main.ts", "throw new Error('not executed')"),
        ("requirements.txt", "packaging>=20\n"),
    ]));
    let before = f
        .executor
        .block_on(f.runtime.status(ADMIN, record.id.clone(), record.revision))
        .unwrap();
    assert!(before.scripts_present);
    assert!(!before.environment_ready);
    let checked = f
        .executor
        .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision))
        .unwrap();
    let evidence = checked.evidence.unwrap();
    assert!(evidence.ready, "{:?}", evidence.missing);
    assert_eq!(
        evidence.runtimes,
        vec!["javascript", "python", "shell", "typescript"]
    );
    assert_eq!(evidence.image_id, f.sandbox.image_id());
    assert!(evidence.versions.contains_key("python:packaging"));
    assert!(!checked.script_execution_ready);
    let reopened = SkillRuntime::open(
        f._root.path().join("evidence"),
        f.skills.clone(),
        f.registry.clone(),
        f.sandbox.clone(),
    )
    .unwrap();
    assert!(
        f.executor
            .block_on(reopened.status(ADMIN, record.id.clone(), record.revision))
            .unwrap()
            .environment_ready
    );
    // The immutable uploaded script remains exactly intact.
    let snapshot = f
        .skills
        .package_snapshot(&ADMIN, &record.id, record.revision)
        .unwrap();
    assert_eq!(
        snapshot.files["scripts/main.py"],
        b"raise RuntimeError('uploaded code must not run')"
    );
    f.executor.block_on(f.sandbox.stop()).unwrap();
}

#[test]
fn missing_packages_and_python_version_are_not_reported_ready_or_installed() {
    let f = fixture();
    let record = f.enable(&f.import(&[
        ("scripts/main.py", "print('no execution')"),
        (
            "requirements.txt",
            "exoharness-test-package-that-does-not-exist==1.0\n",
        ),
        (
            "pyproject.toml",
            "[project]\nname='fixture'\nversion='1.0'\nrequires-python='>=99'\ndependencies=[]\n",
        ),
    ]));
    let checked = f
        .executor
        .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision))
        .unwrap();
    assert!(!checked.environment_ready);
    assert!(!checked.script_execution_ready);
    let evidence = checked.evidence.unwrap();
    assert!(
        evidence
            .missing
            .iter()
            .any(|s| s.starts_with("python-version:"))
    );
    assert!(
        evidence
            .missing
            .iter()
            .any(|s| s.contains("exoharness-test-package"))
    );
    assert_eq!(evidence.image_id, f.sandbox.image_id());
    f.executor.block_on(f.sandbox.stop()).unwrap();
}

#[test]
fn prepare_requires_enable_and_explicit_grants_and_cannot_reuse_a_changed_revision() {
    let f = fixture();
    let record = f.import(&[("main.py", "print(1)")]);
    assert!(matches!(
        f.executor
            .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision)),
        Err(RuntimeError::Permission)
    ));
    assert!(matches!(
        f.executor.block_on(f.runtime.prepare(
            RegistryAccess::User("someone".into()),
            record.id.clone(),
            record.revision
        )),
        Err(RuntimeError::Permission)
    ));
    let record = f.enable(&record);
    let checked = f
        .executor
        .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision))
        .unwrap();
    assert!(checked.environment_ready);
    let updated = f
        .skills
        .import(
            &ADMIN,
            &request(&[("main.py", "print(2)")], Some(record.revision)),
            "test-admin",
        )
        .unwrap();
    assert!(
        f.executor
            .block_on(f.runtime.status(ADMIN, record.id.clone(), record.revision))
            .is_err()
    );
    assert!(
        !f.executor
            .block_on(
                f.runtime
                    .status(ADMIN, updated.id.clone(), updated.revision)
            )
            .unwrap()
            .environment_ready
    );
    f.registry
        .0
        .decide_capability_permission(
            &ADMIN,
            &updated.id,
            updated.revision,
            "sandbox.execute",
            PermissionDecision::Deny,
            "test-admin",
        )
        .unwrap();
    assert!(matches!(
        f.executor.block_on(
            f.runtime
                .prepare(ADMIN, updated.id.clone(), updated.revision)
        ),
        Err(RuntimeError::Permission)
    ));
    f.skills
        .remove(&ADMIN, &updated.id, updated.revision, "test-admin")
        .unwrap();
    assert!(
        f.executor
            .block_on(f.runtime.status(ADMIN, updated.id, updated.revision))
            .is_err()
    );
    f.executor.block_on(f.sandbox.stop()).unwrap();
}

#[test]
fn plain_instruction_skills_do_not_require_python_or_create_probe_jobs() {
    let f = fixture();
    let record = f.import(&[]);
    let checked = f
        .executor
        .block_on(f.runtime.prepare(ADMIN, record.id, record.revision))
        .unwrap();
    assert!(!checked.scripts_present);
    assert!(!checked.environment_ready);
    assert!(checked.evidence.is_none());
    assert_eq!(
        std::fs::read_dir(f._root.path().join("sandbox/jobs"))
            .unwrap()
            .count(),
        0
    );
    f.executor.block_on(f.sandbox.stop()).unwrap();
}

#[test]
fn unsupported_install_options_cannot_escape_into_an_installer_or_host_shell() {
    let f = fixture();
    let record = f.enable(&f.import(&[
        ("main.py", "print(1)"),
        (
            "requirements.txt",
            "--index-url https://example.invalid/private\n",
        ),
    ]));
    assert!(matches!(
        f.executor
            .block_on(f.runtime.prepare(ADMIN, record.id.clone(), record.revision)),
        Err(RuntimeError::Unavailable)
    ));
    assert!(
        !f.executor
            .block_on(f.runtime.status(ADMIN, record.id, record.revision))
            .unwrap()
            .environment_ready
    );
    f.executor.block_on(f.sandbox.stop()).unwrap();
}
