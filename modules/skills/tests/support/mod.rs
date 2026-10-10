use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_registry::Registry;
use exoharness_skills::{ImportRequest, Skills};
use exoharness_storage_postgres::PostgresStore;
use serde_json::json;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
pub fn fixture() -> (tempfile::TempDir, Arc<Registry>, Arc<Skills>) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let url =
        std::env::var("EXOHARNESS_PRO_TEST_DATABASE_URL").expect("isolated PostgreSQL required");
    let namespace = format!(
        "pro_skills_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let store = PostgresStore::connect(&url, &namespace).unwrap();
    store.migrate_registry().unwrap();
    let registry = Arc::new(Registry(store));
    let root = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let skills = Arc::new(Skills::open(root.path().into(), registry.clone()).unwrap());
    (root, registry, skills)
}
pub fn request(body: &str) -> ImportRequest {
    serde_json::from_value(json!({"files":[{"path":"demo/SKILL.md","content_base64":STANDARD.encode(format!("---\nname: demo\ndescription: Test-only Skill\n---\n{body}"))},{"path":"demo/scripts/check.py","content_base64":STANDARD.encode("raise RuntimeError('must not execute on import')")}]})).unwrap()
}
