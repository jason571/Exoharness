use exoharness_contracts::capability::*;
use exoharness_storage_postgres::{PostgresStore, RegistryAccess, StoreError};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};

fn database() -> (PostgresStore, String, String) {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("disposable PostgreSQL required");
    let schema = format!(
        "pro_registry_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = PostgresStore::connect(&url, &schema).unwrap();
    store.migrate_registry().unwrap();
    (store, url, schema)
}
fn input(name: &str, scope: CapabilityScope, permissions: &[&str]) -> ValidatedCapability {
    ValidatedCapability::new(
        CapabilityManifest {
            schema_version: 1,
            name: name.into(),
            version: "1.0.0".into(),
            description: "Test-only instructions; no execution".into(),
            scope,
            source: CapabilitySource::Uploaded {
                file_name: "SKILL.md".into(),
            },
            permissions: permissions.iter().map(|s| (*s).into()).collect(),
            secret_refs: vec![],
            spec: CapabilitySpec::Skill(SkillSpec {
                entrypoint: "SKILL.md".into(),
                content_root: ".".into(),
                executable_scripts: false,
            }),
        },
        Some("a".repeat(64)),
    )
    .unwrap()
}
const ADMIN: RegistryAccess = RegistryAccess::Administrator;

#[test]
fn restored_current_revision_updates_after_history_without_reusing_or_deleting_versions() {
    let (store, url, schema) = database();
    let record = store
        .register_capability(
            &ADMIN,
            input("restored", CapabilityScope::Global, &["read"]),
            "admin",
            true,
        )
        .unwrap();
    store
        .decide_capability_permission(
            &ADMIN,
            &record.id,
            record.revision,
            "read",
            PermissionDecision::Allow,
            "admin",
        )
        .unwrap();
    let mut conn = url
        .parse::<postgres::Config>()
        .unwrap()
        .connect(postgres::NoTls)
        .unwrap();
    conn.batch_execute(&format!("SET search_path={schema}"))
        .unwrap();
    // Reproduce a restored current pointer with later immutable history.
    conn.execute("INSERT INTO capability_versions(capability_id,revision,version,manifest_json,manifest_digest,source_digest,created_at) SELECT capability_id,7,version,manifest_json,manifest_digest,source_digest,created_at FROM capability_versions WHERE capability_id=$1 AND revision=1", &[&record.id]).unwrap();
    let permissions = store.capability_permissions(&ADMIN, &record.id).unwrap();
    let updated = store
        .update_capability(
            &ADMIN,
            &record.id,
            input("restored", CapabilityScope::Global, &["read"]),
            record.revision,
            "admin",
        )
        .unwrap();
    assert_eq!(updated.revision, 8);
    assert!(updated.enabled);
    assert_eq!(
        store.capability_permissions(&ADMIN, &record.id).unwrap(),
        permissions
    );
    let revisions: Vec<_> = store
        .capability_versions(&ADMIN, &record.id)
        .unwrap()
        .iter()
        .map(|v| v.revision)
        .collect();
    assert_eq!(revisions.len(), 3);
    for revision in [1, 7, 8] {
        assert!(revisions.contains(&revision));
    }
    assert!(matches!(
        store.update_capability(
            &ADMIN,
            &record.id,
            input("restored", CapabilityScope::Global, &["read"]),
            record.revision,
            "admin"
        ),
        Err(StoreError::Conflict)
    ));
}

#[test]
fn register_reconnect_migration_and_versions_preserve_exact_contract() {
    let (store, url, schema) = database();
    let record = store
        .register_capability(
            &ADMIN,
            input("skill", CapabilityScope::Global, &["workspace.read"]),
            "admin",
            false,
        )
        .unwrap();
    assert_eq!(record.revision, 1);
    assert!(!record.enabled);
    assert!(matches!(
        store.register_capability(
            &ADMIN,
            input("skill", CapabilityScope::Global, &[]),
            "admin",
            false
        ),
        Err(StoreError::Conflict)
    ));
    let reopened = PostgresStore::connect(&url, &schema).unwrap();
    reopened.migrate_registry().unwrap();
    assert_eq!(reopened.capability(&ADMIN, &record.id).unwrap(), record);
    assert_eq!(
        reopened
            .capability_versions(&ADMIN, &record.id)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        reopened
            .capability_health(&ADMIN, &record.id)
            .unwrap()
            .unwrap()
            .status,
        HealthStatus::Unknown
    );
    assert_eq!(
        reopened.capability_views(&ADMIN, 10).unwrap()[0].permissions[0].decision,
        PermissionDecision::Pending
    );
}
#[test]
fn user_scope_prevents_foreign_visibility_global_mutation_and_self_grants() {
    let (store, _, _) = database();
    let one = RegistryAccess::User("one".into());
    let two = RegistryAccess::User("two".into());
    let global = store
        .register_capability(
            &ADMIN,
            input("global", CapabilityScope::Global, &[]),
            "admin",
            false,
        )
        .unwrap();
    let owned = store
        .register_capability(
            &one,
            input(
                "private",
                CapabilityScope::User {
                    user_id: "one".into(),
                },
                &["network"],
            ),
            "one",
            false,
        )
        .unwrap();
    assert_eq!(store.capabilities(&two, 100).unwrap(), vec![global.clone()]);
    assert!(matches!(
        store.capability(&two, &owned.id),
        Err(StoreError::NotFound)
    ));
    assert!(matches!(
        store.register_capability(
            &one,
            input(
                "other",
                CapabilityScope::User {
                    user_id: "two".into()
                },
                &[]
            ),
            "one",
            false
        ),
        Err(StoreError::NotAuthorized)
    ));
    assert!(matches!(
        store.set_capability_enabled(&one, &global.id, 1, true, "one"),
        Err(StoreError::NotAuthorized)
    ));
    assert!(matches!(
        store.decide_capability_permission(
            &one,
            &owned.id,
            1,
            "network",
            PermissionDecision::Allow,
            "one"
        ),
        Err(StoreError::NotAuthorized)
    ));
    assert!(matches!(
        store.registry_audit(&one, 10),
        Err(StoreError::NotAuthorized)
    ));
}
#[test]
fn update_cas_retains_only_same_permission_grants_and_invalidates_old_health() {
    let (store, url, schema) = database();
    let record = store
        .register_capability(
            &ADMIN,
            input("skill", CapabilityScope::Global, &["read"]),
            "admin",
            false,
        )
        .unwrap();
    store
        .decide_capability_permission(
            &ADMIN,
            &record.id,
            1,
            "read",
            PermissionDecision::Allow,
            "admin",
        )
        .unwrap();
    let mut conn = url
        .parse::<postgres::Config>()
        .unwrap()
        .connect(postgres::NoTls)
        .unwrap();
    conn.batch_execute(&format!(
        "SET search_path={schema};UPDATE capability_health SET status='healthy'"
    ))
    .unwrap();
    let updated = store
        .update_capability(
            &ADMIN,
            &record.id,
            input("skill", CapabilityScope::Global, &["read", "write"]),
            1,
            "admin",
        )
        .unwrap();
    assert_eq!(updated.revision, 2);
    let decisions = store.capability_permissions(&ADMIN, &record.id).unwrap();
    assert_eq!(decisions[0].decision, PermissionDecision::Allow);
    assert_eq!(decisions[1].decision, PermissionDecision::Pending);
    assert_eq!(
        store
            .capability_health(&ADMIN, &record.id)
            .unwrap()
            .unwrap()
            .status,
        HealthStatus::Unknown
    );
    assert!(matches!(
        store.update_capability(
            &ADMIN,
            &record.id,
            input("skill", CapabilityScope::Global, &[]),
            1,
            "admin"
        ),
        Err(StoreError::Conflict)
    ));
    assert!(matches!(
        store.update_capability(
            &ADMIN,
            &record.id,
            input("renamed", CapabilityScope::Global, &[]),
            2,
            "admin"
        ),
        Err(StoreError::Invalid(_))
    ));
    assert_eq!(
        store.capability_versions(&ADMIN, &record.id).unwrap().len(),
        2
    );
    assert_eq!(store.capability(&ADMIN, &record.id).unwrap(), updated);
}
#[test]
fn delete_reimport_never_recycles_revision_and_cascades_owned_records() {
    let (store, _, _) = database();
    let record = store
        .register_capability(
            &ADMIN,
            input("skill", CapabilityScope::Global, &["read"]),
            "admin",
            false,
        )
        .unwrap();
    let enabled = store
        .set_capability_enabled(&ADMIN, &record.id, 1, true, "admin")
        .unwrap();
    let unchanged = store
        .set_capability_enabled(&ADMIN, &record.id, enabled.revision, true, "admin")
        .unwrap();
    assert_eq!(enabled, unchanged);
    store
        .remove_capability(&ADMIN, &record.id, enabled.revision, "admin")
        .unwrap();
    assert!(matches!(
        store.capability_versions(&ADMIN, &record.id),
        Err(StoreError::NotFound)
    ));
    let recreated = store
        .register_capability(
            &ADMIN,
            input("skill", CapabilityScope::Global, &[]),
            "admin",
            false,
        )
        .unwrap();
    assert!(recreated.revision > enabled.revision);
    assert!(
        store
            .capability_permissions(&ADMIN, &record.id)
            .unwrap()
            .is_empty()
    );
    assert_eq!(store.registry_audit(&ADMIN, 10).unwrap().len(), 4);
}
#[test]
fn concurrent_registration_has_one_commit_and_audit_failure_rolls_back_all_state() {
    let (store, url, schema) = database();
    let barrier = Arc::new(Barrier::new(2));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let store = store.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.register_capability(
                    &ADMIN,
                    input("same", CapabilityScope::Global, &[]),
                    "admin",
                    false,
                )
            })
        })
        .collect();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(store.capabilities(&ADMIN, 10).unwrap().len(), 1);
    assert_eq!(store.registry_audit(&ADMIN, 10).unwrap().len(), 1);
    let mut conn = url
        .parse::<postgres::Config>()
        .unwrap()
        .connect(postgres::NoTls)
        .unwrap();
    conn.batch_execute(&format!("SET search_path={schema};ALTER TABLE registry_audit ADD CONSTRAINT reject_actor CHECK(actor<>'rejected')")).unwrap();
    assert!(
        store
            .register_capability(
                &ADMIN,
                input("rejected", CapabilityScope::Global, &[]),
                "rejected",
                false
            )
            .is_err()
    );
    assert_eq!(store.capabilities(&ADMIN, 10).unwrap().len(), 1);
    let record = store.capabilities(&ADMIN, 10).unwrap().remove(0);
    assert!(
        store
            .set_capability_enabled(&ADMIN, &record.id, record.revision, true, "rejected")
            .is_err()
    );
    assert_eq!(store.capability(&ADMIN, &record.id).unwrap(), record);
}
