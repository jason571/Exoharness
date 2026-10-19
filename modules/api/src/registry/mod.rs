// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Registry HTTP composition; imports and execution belong to their modules.
mod assistants;
mod details;
mod mcp_outputs;
mod preparation;
mod reads;
mod removal;
mod secrets;
mod skill_runtime;
mod validation;
mod writes;
use crate::{ApiState, Surface, auth};
use axum::{Router, middleware, routing::get};
use exoharness_registry::Registry;
use std::sync::Arc;

#[derive(Clone)]
struct RegistryState {
    registry: Arc<Registry>,
    skills: Arc<exoharness_skills::Skills>,
    mcp: Arc<exoharness_mcp::Mcp>,
    runtime: Option<Arc<exoharness_skill_runtime::SkillRuntime>>,
}
pub fn registry_router(
    api: ApiState,
    registry: Arc<Registry>,
    skills: Arc<exoharness_skills::Skills>,
    mcp: Arc<exoharness_mcp::Mcp>,
) -> Router {
    registry_router_with_runtime(api, registry, skills, mcp, None)
}
pub fn registry_router_with_runtime(
    api: ApiState,
    registry: Arc<Registry>,
    skills: Arc<exoharness_skills::Skills>,
    mcp: Arc<exoharness_mcp::Mcp>,
    runtime: Option<Arc<exoharness_skill_runtime::SkillRuntime>>,
) -> Router {
    if api.surface != Surface::Management {
        return Router::new();
    }
    Router::new()
        .route("/v1/audit", get(details::audit))
        .route("/v1/capabilities", get(reads::records))
        .route("/v1/capabilities/overview", get(reads::overview))
        .route("/v1/assistant-capabilities", get(assistants::capabilities))
        .route(
            "/v1/capabilities/{id}",
            get(details::record).delete(removal::remove),
        )
        .route(
            "/v1/capabilities/{id}/permissions",
            get(details::permissions),
        )
        .route(
            "/v1/capabilities/{id}/artifact-origins",
            axum::routing::put(mcp_outputs::update),
        )
        .route(
            "/v1/capabilities/{id}/permissions/{permission}",
            axum::routing::put(writes::permission),
        )
        .route(
            "/v1/capabilities/{id}/enabled",
            axum::routing::put(writes::enabled),
        )
        .route("/v1/capabilities/{id}/health", get(details::health))
        .route("/v1/capabilities/{id}/secrets", get(details::secrets))
        .route(
            "/v1/capabilities/{id}/secrets/{name}",
            axum::routing::put(secrets::update).delete(secrets::clear),
        )
        .route(
            "/v1/capabilities/{id}/preparation",
            get(preparation::status).post(preparation::prepare),
        )
        .route(
            "/v1/capabilities/{id}/validate",
            axum::routing::post(validation::validate),
        )
        .route("/v1/capabilities/{id}/tools", get(validation::tools))
        .route("/v1/capabilities/{id}/runtime", get(details::runtime))
        .route("/v1/capabilities/{id}/versions", get(details::versions))
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .with_state(RegistryState {
            registry,
            skills,
            mcp,
            runtime,
        })
}
