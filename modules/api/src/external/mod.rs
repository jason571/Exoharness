// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Application delegation is isolated from browser/static-key authentication.
mod authorization;
mod conversations;
mod gate;
mod management;
mod sessions;
mod tokens;
use crate::{ApiState, Surface, auth, identity};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post},
};
use exoharness_configuration::{Assistants, Models};
use exoharness_identity::AuthStore;
use exoharness_model_runtime::ModelRuntime;
use std::sync::Arc;

#[derive(Clone)]
struct ExternalState {
    api: ApiState,
    identity: Arc<AuthStore>,
    assistants: Arc<Assistants>,
}
pub fn external_management_router(api: ApiState, assistants: Arc<Assistants>) -> Router {
    let Some(identity) = api
        .credentials
        .identity
        .clone()
        .filter(|_| api.surface == Surface::Management)
    else {
        return Router::new();
    };
    let state = ExternalState {
        api: api.clone(),
        identity,
        assistants,
    };
    Router::new()
        .route(
            "/v1/management/external-conversations",
            get(conversations::list),
        )
        .route(
            "/v1/management/external-applications/{client}/sessions/{session}",
            get(conversations::detail),
        )
        .route(
            "/v1/management/external-applications",
            get(management::list).post(management::create),
        )
        .route(
            "/v1/management/external-applications/{client}",
            axum::routing::put(management::update).delete(management::remove),
        )
        .route(
            "/v1/management/external-applications/{client}/key",
            post(management::view_key),
        )
        .route(
            "/v1/management/external-applications/{client}/grants",
            get(management::grants),
        )
        .route(
            "/v1/management/external-applications/{client}/grants/{grant}",
            axum::routing::delete(management::revoke_grant),
        )
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .layer(DefaultBodyLimit::max(32 * 1024))
        .with_state(state)
}
pub fn external_router(
    api: ApiState,
    models: Arc<Models>,
    assistants: Arc<Assistants>,
    runtime: Arc<ModelRuntime>,
    tasks: Arc<crate::ChatTasks>,
    tools: Option<Arc<exoharness_tools::ToolRuntime>>,
) -> Router {
    let Some(identity) = api
        .credentials
        .identity
        .clone()
        .filter(|_| api.surface == Surface::Portal)
    else {
        return Router::new();
    };
    let state = ExternalState {
        api: api.clone(),
        identity,
        assistants: assistants.clone(),
    };
    let browser = Router::new()
        .route("/v1/external/authorize", post(authorization::approve))
        .route("/v1/external/grants", get(authorization::grants))
        .route(
            "/v1/external/grants/{grant}",
            axum::routing::delete(authorization::revoke),
        )
        .layer(middleware::from_fn_with_state(api.clone(), auth::gate))
        .with_state(state.clone());
    let delegated = Router::new()
        .route("/v1/external/me", get(sessions::me))
        .route("/v1/external/sessions", post(sessions::create))
        .route("/v1/external/sessions/{session}", get(sessions::detail))
        .route(
            "/v1/external/sessions/{session}/device",
            axum::routing::put(sessions::device),
        )
        .with_state(state.clone())
        .merge(crate::chat::external_chat_router(
            api, models, assistants, runtime, tasks, tools,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), gate::gate));
    let public = Router::new()
        .route("/v1/external/authorize", get(authorization::page))
        .route("/v1/external/token", post(tokens::exchange))
        .route("/v1/external/revoke", post(tokens::revoke))
        .with_state(state);
    public
        .merge(browser)
        .merge(delegated)
        .layer(DefaultBodyLimit::max(32 * 1024))
}
async fn work<T, F>(state: &ExternalState, f: F) -> Result<T, identity::IdentityError>
where
    T: Send + 'static,
    F: FnOnce(&AuthStore) -> Result<T, exoharness_identity::AuthError> + Send + 'static,
{
    identity::work(state.identity.clone(), f).await
}
async fn assistant(
    state: &ExternalState,
    id: &str,
) -> Result<serde_json::Value, identity::IdentityError> {
    let assistants = state.assistants.clone();
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        assistants
            .refresh()
            .map_err(|_| identity::IdentityError::unavailable())?;
        let profile = assistants.delegated_profile(&id);
        if profile.is_null() {
            return Err(identity::IdentityError::new(
                axum::http::StatusCode::CONFLICT,
                "assistant_unavailable",
                "Application assistant is unavailable",
            ));
        }
        Ok(profile)
    })
    .await
    .map_err(|_| identity::IdentityError::unavailable())?
}
