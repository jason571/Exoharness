//! Isolated temporary catalog fixtures shared by configuration tests.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

pub(crate) struct Fixture(pub(crate) PathBuf);
impl Fixture {
    pub(crate) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "exoharness-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path)
    }
    pub(crate) fn write(&self, name: &str, value: &Value) {
        self.bytes(name, value.to_string().as_bytes());
    }
    pub(crate) fn bytes(&self, name: &str, bytes: &[u8]) {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
            .open(self.0.join(name))
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
pub(crate) fn model() -> Value {
    json!({"providers":[{"name":"api","api":"openai_compatible","model_type":"llm","base_url":"https://example.test/v1","enabled":true,"api_key_configured":false}],"models":[{"name":"测试模型","model_id":"fixture","display_name":"测试模型","provider":"api","model_type":"llm","enabled":true,"context_window":128000,"max_tokens":16384,"thinking":"none","temperature":0.7}],"default_model":"测试模型","default_models":{"llm":"测试模型"}})
}
pub(crate) fn assistants() -> Value {
    json!({"default_profile":"test","profiles":[{"id":"test","enabled":true,"active_revision":2,"created_at":1,"updated_at":2,"revisions":[snapshot(1),snapshot(2)]}]})
}
pub(crate) fn snapshot(revision: u64) -> Value {
    json!({"id":"test","revision":revision,"display_name":"Test","avatar_text":"T","description":"test","system_prompt":"Private assistant instructions.\nSecond line.","skill_ids":["skill:global:_:test"],"access":{"skills":{"mode":"selected","ids":["skill:global:_:test"]},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"all"},"published":true},"digest":"sha256:legacy-digest-kept","created_at":revision})
}
