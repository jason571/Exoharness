//! Original console contracts backed by real, scoped PostgreSQL records.
mod categories;
mod documents;
mod imports;
mod libraries;
mod memories;

use crate::{ApiState, Surface, auth, errors::ApiError};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    http::StatusCode,
    middleware,
    routing::{get, put},
};
use exoharness_configuration::Assistants;
use exoharness_storage_postgres::PostgresStore;
use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
struct KnowledgeState {
    store: PostgresStore,
    assistants: Arc<Assistants>,
    surface: Surface,
    sandbox: imports::ParserSandbox,
}
type Params = HashMap<String, String>;
fn scope(
    state: &KnowledgeState,
    principal: &auth::Principal,
    params: &Params,
) -> Result<String, ApiError> {
    if let Some(library) = params.get("library") {
        if state.surface != Surface::Management || principal.role != auth::Role::Admin {
            return Err(ApiError(
                StatusCode::FORBIDDEN,
                "administrator_required",
                "公共资料库仅由管理员管理",
            ));
        }
        Ok(format!("library:{library}"))
    } else {
        // Never accept an owner id supplied by a browser.
        Ok(principal.id.clone())
    }
}
fn id(params: &Params) -> Result<&str, ApiError> {
    params.get("id").map(String::as_str).ok_or(ApiError(
        StatusCode::BAD_REQUEST,
        "missing_id",
        "缺少记录标识",
    ))
}
fn new_id(prefix: &str) -> Result<String, ApiError> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_failed",
            "无法创建安全标识",
        )
    })?;
    Ok(format!(
        "{prefix}-{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    ))
}
pub fn knowledge_router(auth_state: ApiState, assistants: Arc<Assistants>) -> Router {
    knowledge_router_with_sandbox(auth_state, assistants, None)
}
pub fn knowledge_router_with_sandbox(
    auth_state: ApiState,
    assistants: Arc<Assistants>,
    sandbox: imports::ParserSandbox,
) -> Router {
    let mut routes = Router::new()
        .route(
            "/v1/knowledge/categories",
            get(categories::list).post(categories::create),
        )
        .route(
            "/v1/knowledge/categories/{id}",
            put(categories::update).delete(categories::delete),
        )
        .route(
            "/v1/knowledge/documents",
            get(documents::list).post(documents::create),
        )
        .route(
            "/v1/knowledge/documents/{id}",
            get(documents::detail)
                .put(documents::update)
                .delete(documents::delete),
        )
        .route("/v1/knowledge/search", get(documents::search))
        .route(
            "/v1/knowledge/documents/import",
            axum::routing::post(imports::import),
        )
        .route("/v1/knowledge/documents/{id}/blocks", get(imports::blocks))
        .route(
            "/v1/knowledge/documents/{id}/blocks/{block}",
            get(imports::block),
        )
        .route("/v1/memories", get(memories::list).post(memories::create))
        .route(
            "/v1/memories/{id}",
            put(memories::update).delete(memories::delete),
        );
    if auth_state.surface == Surface::Management {
        routes = routes
            .route(
                "/v1/knowledge/libraries",
                get(libraries::list).post(libraries::create),
            )
            .route(
                "/v1/knowledge/libraries/{library}",
                get(libraries::detail)
                    .put(libraries::update)
                    .delete(libraries::delete),
            )
            .route(
                "/v1/knowledge/libraries/{library}/grants",
                put(libraries::grants),
            )
            .route(
                "/v1/knowledge/libraries/{library}/categories",
                get(categories::list).post(categories::create),
            )
            .route(
                "/v1/knowledge/libraries/{library}/categories/{id}",
                put(categories::update).delete(categories::delete),
            )
            .route(
                "/v1/knowledge/libraries/{library}/documents",
                get(documents::list).post(documents::create),
            )
            .route(
                "/v1/knowledge/libraries/{library}/documents/{id}",
                get(documents::detail)
                    .put(documents::update)
                    .delete(documents::delete),
            )
            .route(
                "/v1/knowledge/libraries/{library}/documents/import",
                axum::routing::post(imports::import),
            )
            .route(
                "/v1/knowledge/libraries/{library}/documents/{id}/blocks",
                get(imports::blocks),
            )
            .route(
                "/v1/knowledge/libraries/{library}/documents/{id}/blocks/{block}",
                get(imports::block),
            );
    }
    let state = KnowledgeState {
        store: auth_state.execution.0.clone(),
        assistants,
        surface: auth_state.surface,
        sandbox,
    };
    routes
        .layer(DefaultBodyLimit::max(6 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(auth_state, auth::gate))
        .with_state(state)
}
