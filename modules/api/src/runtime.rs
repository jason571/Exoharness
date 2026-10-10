//! Runtime fact projection, not a grant to execute untrusted source.
use crate::{ApiState, Surface, auth};
use axum::{Json, Router, middleware, routing::get};
use exoharness_sandbox::Sandbox;
use serde_json::json;
use std::sync::Arc;
pub fn runtime_router(api: ApiState, sandbox: Option<Arc<Sandbox>>) -> Router {
    runtime_router_with_tools(api, sandbox, None)
}
pub fn runtime_router_with_tools(
    api: ApiState,
    sandbox: Option<Arc<Sandbox>>,
    tools: Option<Arc<exoharness_tools::ToolRuntime>>,
) -> Router {
    if api.surface != Surface::Management {
        return Router::new();
    }
    Router::new().route("/v1/local/runtime",get(move||{
        let sandbox=sandbox.clone();async move {
            Json(json!({"sandbox_ready":sandbox.is_some(),"image_id":sandbox.as_ref().map(|s|s.image_id()),"backend":"linux_oci","runtimes":["python","shell","javascript","typescript"],"network":"disabled","agent_integrated":tools.is_some()}))
        }
    })).layer(middleware::from_fn_with_state(api,auth::gate))
}
