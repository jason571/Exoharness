mod support;
use exoharness_contracts::capability::{CapabilityScope, PermissionDecision};
use exoharness_skills::{SkillError, Skills};
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use support::{fixture, request};
const ADMIN: RegistryAccess = RegistryAccess::Administrator;
#[test]
fn preview_is_pure_import_is_immutable_and_reload_has_exact_instructions() {
    let (root, registry, skills) = fixture();
    let input = request("Exact instructions 中文\n");
    let preview = skills.preview(&ADMIN, &input).unwrap();
    assert_eq!(preview.action, "create");
    assert!(registry.0.capabilities(&ADMIN, 100).unwrap().is_empty());
    assert_eq!(
        std::fs::read_dir(root.path().join("packages"))
            .unwrap()
            .count(),
        0
    );
    let record = skills.import(&ADMIN, &input, "admin").unwrap();
    assert!(!record.enabled);
    let instructions = skills.instructions(&ADMIN, &record.id).unwrap();
    assert!(instructions.contains("Exact instructions 中文"));
    assert_eq!(skills.import(&ADMIN, &input, "admin").unwrap(), record);
    assert_eq!(skills.preview(&ADMIN, &input).unwrap().action, "unchanged");
    let restarted = Skills::open(root.path().into(), registry.clone()).unwrap();
    assert_eq!(
        restarted.instructions(&ADMIN, &record.id).unwrap(),
        instructions
    );
    assert_eq!(restarted.files(&ADMIN, &record.id).unwrap().len(), 2);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir = root
            .path()
            .join("packages")
            .join(format!("{:x}", Sha256::digest(record.id.as_bytes())))
            .join(record.source_digest.unwrap());
        assert_eq!(
            std::fs::metadata(dir.join("SKILL.md"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o400
        );
    }
}
#[test]
fn updates_require_current_revision_and_preserve_explicit_grants() {
    let (_root, registry, skills) = fixture();
    let mut input = request("First");
    input.permissions = vec!["workspace.read".into()];
    let record = skills.import(&ADMIN, &input, "admin").unwrap();
    registry
        .0
        .decide_capability_permission(
            &ADMIN,
            &record.id,
            record.revision,
            "workspace.read",
            PermissionDecision::Allow,
            "admin",
        )
        .unwrap();
    let enabled = registry
        .0
        .set_capability_enabled(&ADMIN, &record.id, record.revision, true, "admin")
        .unwrap();
    let mut next = request("Changed");
    next.permissions = input.permissions.clone();
    assert!(matches!(
        skills.import(&ADMIN, &next, "admin"),
        Err(SkillError::Store(StoreError::Conflict))
    ));
    let preview = skills.preview(&ADMIN, &next).unwrap();
    assert_eq!(preview.action, "update");
    next.expected_revision = Some(enabled.revision);
    let changed = skills.import(&ADMIN, &next, "admin").unwrap();
    assert!(changed.enabled);
    assert!(changed.revision > enabled.revision);
    assert_eq!(
        registry
            .0
            .capability_permissions(&ADMIN, &record.id)
            .unwrap()[0]
            .decision,
        PermissionDecision::Allow
    );
    assert!(
        skills
            .instructions(&ADMIN, &record.id)
            .unwrap()
            .contains("Changed")
    );
    assert!(matches!(
        skills.import(&ADMIN, &next, "admin"),
        Err(SkillError::Store(StoreError::Conflict))
    ));
}
#[test]
fn scope_is_checked_before_disk_writes_and_user_packages_are_isolated() {
    let (root, _, skills) = fixture();
    let one = RegistryAccess::User("one".into());
    let two = RegistryAccess::User("two".into());
    let mut input = request("Owned");
    assert!(matches!(
        skills.import(&one, &input, "one"),
        Err(SkillError::Store(StoreError::NotAuthorized))
    ));
    assert_eq!(
        std::fs::read_dir(root.path().join("packages"))
            .unwrap()
            .count(),
        0
    );
    input.scope = CapabilityScope::User {
        user_id: "one".into(),
    };
    let record = skills.import(&one, &input, "one").unwrap();
    assert!(skills.instructions(&two, &record.id).is_err());
    assert!(
        skills
            .remove(&two, &record.id, record.revision, "two")
            .is_err()
    );
    input.scope = CapabilityScope::User {
        user_id: "two".into(),
    };
    let other = skills.import(&two, &input, "two").unwrap();
    assert_ne!(other.id, record.id);
    skills
        .remove(&one, &record.id, record.revision, "one")
        .unwrap();
    assert!(skills.instructions(&two, &other.id).is_ok());
}
#[test]
fn deletion_removes_all_revisions_and_reimport_never_recycles_revision() {
    let (root, registry, skills) = fixture();
    let record = skills.import(&ADMIN, &request("First"), "admin").unwrap();
    let mut next = request("Second");
    next.expected_revision = Some(record.revision);
    let next = skills.import(&ADMIN, &next, "admin").unwrap();
    assert!(
        skills
            .remove(&ADMIN, &record.id, record.revision, "admin")
            .is_err()
    );
    assert!(skills.instructions(&ADMIN, &record.id).is_ok());
    skills
        .remove(&ADMIN, &record.id, next.revision, "admin")
        .unwrap();
    assert!(matches!(
        registry.0.capability(&ADMIN, &record.id),
        Err(StoreError::NotFound)
    ));
    assert_eq!(
        std::fs::read_dir(root.path().join("packages"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(root.path().join("removing"))
            .unwrap()
            .count(),
        0
    );
    assert!(
        skills
            .import(&ADMIN, &request("First"), "admin")
            .unwrap()
            .revision
            > next.revision
    );
}
#[test]
fn concurrent_same_import_is_idempotent_and_drift_is_not_silently_read() {
    let (root, registry, skills) = fixture();
    let workers = (0..3)
        .map(|_| {
            let skills = skills.clone();
            std::thread::spawn(move || {
                skills
                    .import(&ADMIN, &request("Original"), "admin")
                    .unwrap()
            })
        })
        .collect::<Vec<_>>();
    let records = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect::<Vec<_>>();
    assert!(records.iter().all(|r| r == &records[0]));
    assert_eq!(registry.0.registry_audit(&ADMIN, 100).unwrap().len(), 1);
    let record = &records[0];
    let package = root
        .path()
        .join("packages")
        .join(format!("{:x}", Sha256::digest(record.id.as_bytes())))
        .join(record.source_digest.as_ref().unwrap());
    std::fs::write(package.join("injected.txt"), "drift").unwrap();
    assert!(skills.instructions(&ADMIN, &record.id).is_err());
    assert!(
        skills
            .import(&ADMIN, &request("Original"), "admin")
            .is_err()
    );
}
#[test]
fn crashed_delete_restores_files_if_database_remains_and_removes_if_committed() {
    for committed in [false, true] {
        let (root, registry, skills) = fixture();
        let record = skills
            .import(&ADMIN, &request("Retained"), "admin")
            .unwrap();
        drop(skills);
        let trash = root.path().join("removing/remove-crash");
        std::fs::create_dir(&trash).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&trash, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        std::fs::write(
            trash.join("intent.json"),
            serde_json::json!({"id":record.id}).to_string(),
        )
        .unwrap();
        let live = root
            .path()
            .join("packages")
            .join(format!("{:x}", Sha256::digest(record.id.as_bytes())));
        std::fs::rename(&live, trash.join("packages")).unwrap();
        if committed {
            registry
                .0
                .remove_capability(&ADMIN, &record.id, record.revision, "admin")
                .unwrap();
        }
        let reopened = Skills::open(root.path().into(), Arc::clone(&registry)).unwrap();
        assert!(!trash.exists());
        assert_eq!(
            reopened.instructions(&ADMIN, &record.id).is_ok(),
            !committed
        );
        assert_eq!(live.exists(), !committed);
    }
}
