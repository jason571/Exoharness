//! Configuration HTTP adapters. The router only composes handlers.
mod assistant_authorization;
mod assistant_requests;
mod assistant_writes;
mod avatars;
mod model_requests;
mod model_test;
mod model_writes;
mod models;
mod profiles;
mod scheduler;
mod speech;
use crate::{ApiState, Surface, auth};
use axum::{
    Router, middleware,
    routing::{delete, get, post, put},
};
use exoharness_configuration::{Assistants, Models};
use std::sync::Arc;

#[derive(Clone)]
struct ConfigurationState {
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    management: bool,
    identity: Option<Arc<exoharness_identity::AuthStore>>,
    workers: Arc<tokio::sync::Semaphore>,
    registry: Option<Arc<exoharness_registry::Registry>>,
    runtime: Option<Arc<exoharness_model_runtime::ModelRuntime>>,
    store: exoharness_storage_postgres::PostgresStore,
}
pub fn configuration_router(
    auth_state: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
) -> Router {
    configuration_router_with_registry(auth_state, models, assistants, None)
}
pub fn configuration_router_with_registry(
    auth_state: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    registry: Option<Arc<exoharness_registry::Registry>>,
) -> Router {
    configuration_router_with_runtime(auth_state, models, assistants, registry, None)
}
pub fn configuration_router_with_runtime(
    auth_state: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    registry: Option<Arc<exoharness_registry::Registry>>,
    runtime: Option<Arc<exoharness_model_runtime::ModelRuntime>>,
) -> Router {
    let management = auth_state.surface == Surface::Management;
    let store = auth_state.execution.0.clone();
    let identity = auth_state.credentials.identity.clone();
    let mut routes = Router::new()
        // The reused portal requests this public identity endpoint. Both paths
        // resolve the authenticated user's assignment through the same handler.
        .route("/v1/agent-profile", get(profiles::default_assistant))
        .route("/v1/chat/configuration", get(models::chat_configuration))
        .route("/v1/speech-input/configuration", get(speech::configuration))
        .route(
            "/v1/speech-input",
            post(speech::transcribe).layer(axum::extract::DefaultBodyLimit::max(
                exoharness_model_runtime::MAX_AUDIO_BYTES,
            )),
        )
        .route(
            "/v1/agent-profiles/default",
            get(profiles::default_assistant),
        )
        .route("/v1/agent-assets/{id}", get(avatars::avatar));
    if management {
        routes = routes
            .route(
                "/v1/scheduler-configuration",
                get(scheduler::read).put(scheduler::save),
            )
            .route("/v1/model-catalog", get(models::catalog))
            .route("/v1/model-configuration", post(model_writes::configuration))
            .route("/v1/model-providers", post(model_writes::provider))
            .route(
                "/v1/model-providers/{name}",
                delete(model_writes::remove_provider),
            )
            .route(
                "/v1/model-providers/{name}/enabled",
                put(model_writes::provider_enabled),
            )
            .route("/v1/models", post(model_writes::model))
            .route("/v1/models/{name}", delete(model_writes::remove_model))
            .route(
                "/v1/models/{name}/enabled",
                put(model_writes::model_enabled),
            )
            .route(
                "/v1/models/{name}/default",
                put(model_writes::default_model),
            )
            .route("/v1/models/{name}/test", post(model_test::test))
            .route(
                "/v1/agent-profiles",
                get(profiles::assistant_list).post(assistant_writes::create),
            )
            .route(
                "/v1/agent-profiles/{id}",
                get(profiles::assistant_detail)
                    .put(assistant_writes::update)
                    .delete(assistant_writes::remove),
            )
            .route(
                "/v1/agent-profiles/{id}/enabled",
                put(assistant_writes::enabled),
            )
            .route(
                "/v1/agent-profiles/{id}/default",
                put(assistant_writes::default),
            )
            .route(
                "/v1/agent-assets",
                post(assistant_writes::upload)
                    .layer(axum::extract::DefaultBodyLimit::max(3 * 1024 * 1024)),
            );
    }
    routes
        .layer(middleware::from_fn_with_state(auth_state, auth::gate))
        .with_state(ConfigurationState {
            store,
            models,
            assistants,
            management,
            identity,
            workers: Arc::new(tokio::sync::Semaphore::new(4)),
            registry,
            runtime,
        })
}
