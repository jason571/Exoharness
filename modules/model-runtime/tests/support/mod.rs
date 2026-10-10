pub mod server;
use exoharness_configuration::Models;
use exoharness_model_runtime::{ChatRequest, Message, ModelRuntime, RuntimePolicy, ToolChoice};
use serde_json::json;
use std::sync::Arc;
pub struct Fixture {
    pub root: tempfile::TempDir,
    pub models: Arc<Models>,
    pub runtime: Arc<ModelRuntime>,
    pub server: server::Server,
}
impl Fixture {
    pub async fn new(api: &str, reply: server::Reply, policy: RuntimePolicy) -> Self {
        let root = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let models = Arc::new(Models::initialize(root.path()).unwrap());
        let server = server::Server::new(reply).await;
        models.save_configuration(
            json!({"name":"api","model_type":"llm","api":api,"base_url":server.url,"enabled":true}),
            json!({"name":"llm","provider":"api","model_id":"fixture-model","display_name":"Fixture","model_type":"llm","enabled":true,"max_tokens":512,"context_window":16000,"thinking":"none","temperature":0.2}),
            Some("local-fixture-credential"),None).unwrap();
        let runtime = Arc::new(ModelRuntime::new(models.clone(), policy).unwrap());
        Self {
            root,
            models,
            runtime,
            server,
        }
    }
}
pub fn request() -> ChatRequest {
    ChatRequest {
        messages: vec![
            Message::System {
                text: "Instructions only.".into(),
            },
            Message::User {
                text: "Reply OK.".into(),
            },
        ],
        tools: vec![],
        tool_choice: ToolChoice::None,
        output_token_limit: 128,
    }
}
