// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Session/history and response feedback adapters; execution is a separate module.
mod feedback;
mod sessions;
use crate::{ApiState, Surface, auth};
use axum::{Router, middleware, routing::get};
use exoharness_configuration::Assistants;
use exoharness_storage_postgres::PostgresStore;
use std::sync::Arc;

#[derive(Clone)]
struct ConversationState {
    store: PostgresStore,
    assistants: Arc<Assistants>,
    identity: Option<Arc<exoharness_identity::AuthStore>>,
    management: bool,
}
pub fn conversation_router(auth_state: ApiState, assistants: Arc<Assistants>) -> Router {
    let state = ConversationState {
        store: auth_state.execution.0.clone(),
        assistants,
        identity: auth_state.credentials.identity.clone(),
        management: auth_state.surface == Surface::Management,
    };
    Router::new()
        .route("/v1/sessions", get(sessions::list).post(sessions::create))
        .route(
            "/v1/sessions/{session}",
            get(sessions::detail)
                .patch(sessions::update)
                .delete(sessions::delete),
        )
        .route(
            "/v1/sessions/{session}/response-actions",
            get(feedback::session),
        )
        .route(
            "/v1/runs/{run}/response-actions",
            get(feedback::run).post(feedback::record),
        )
        .layer(middleware::from_fn_with_state(auth_state, auth::gate))
        .with_state(state)
}
