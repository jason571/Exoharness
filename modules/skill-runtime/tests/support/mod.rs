use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_contracts::capability::{PermissionDecision, StoredCapability};
use exoharness_registry::Registry;
use exoharness_sandbox::Sandbox;
use exoharness_skill_runtime::SkillRuntime;
use exoharness_skills::{ImportRequest, Skills};
use exoharness_storage_postgres::{PostgresStore, RegistryAccess};
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub struct Fixture {
    pub _root: tempfile::TempDir,
    pub registry: Arc<Registry>,
    pub skills: Arc<Skills>,
    pub sandbox: Arc<Sandbox>,
    pub runtime: SkillRuntime,
    pub executor: tokio::runtime::Runtime,
}
pub fn fixture() -> Fixture {
    let executor = tokio::runtime::Runtime::new().unwrap();
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let url = std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated database");
    let store = PostgresStore::connect(
        &url,
        &format!(
            "runtime_skill_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ),
    )
    .unwrap();
    store.migrate_registry().unwrap();
    let registry = Arc::new(Registry(store));
    let root = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let sandbox = Arc::new(
        executor
            .block_on(Sandbox::open(
                root.path().join("sandbox"),
                &std::env::var("EXOHARNESS_PRO_TEST_SANDBOX_IMAGE").expect("cached image"),
            ))
            .unwrap(),
    );
    let skills = Arc::new(Skills::open(root.path().join("skills"), registry.clone()).unwrap());
    let runtime = SkillRuntime::open(
        root.path().join("evidence"),
        skills.clone(),
        registry.clone(),
        sandbox.clone(),
    )
    .unwrap();
    Fixture {
        _root: root,
        registry,
        skills,
        sandbox,
        runtime,
        executor,
    }
}
pub fn request(files: &[(&str, &str)], expected: Option<i64>) -> ImportRequest {
    let mut entries = vec![
        json!({"path":"SKILL.md","content_base64":STANDARD.encode("---\nname: runtime-fixture\ndescription: Generic isolated environment test\n---\nUse the tool after it is authorized.")}),
    ];
    entries.extend(
        files
            .iter()
            .map(|(path, bytes)| json!({"path":path,"content_base64":STANDARD.encode(bytes)})),
    );
    serde_json::from_value(
        json!({"files":entries,"permissions":["sandbox.execute"],"expected_revision":expected}),
    )
    .unwrap()
}
impl Fixture {
    pub fn import(&self, files: &[(&str, &str)]) -> StoredCapability {
        self.skills
            .import(
                &RegistryAccess::Administrator,
                &request(files, None),
                "test-admin",
            )
            .unwrap()
    }
    pub fn enable(&self, record: &StoredCapability) -> StoredCapability {
        self.registry
            .0
            .decide_capability_permission(
                &RegistryAccess::Administrator,
                &record.id,
                record.revision,
                "sandbox.execute",
                PermissionDecision::Allow,
                "test-admin",
            )
            .unwrap();
        self.registry
            .0
            .set_capability_enabled(
                &RegistryAccess::Administrator,
                &record.id,
                record.revision,
                true,
                "test-admin",
            )
            .unwrap()
    }
}
