//! Management protocol operations do not expose tool execution to the browser.
mod errors;
mod imports;
use crate::{ApiState, Surface, auth};
use axum::{Router, extract::DefaultBodyLimit, middleware, routing::post};
use exoharness_mcp::{Mcp, McpError};
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;

pub fn mcp_router(api: ApiState, mcp: Arc<Mcp>) -> Router {
    if api.surface != Surface::Management {
        return Router::new();
    }
    Router::new()
        .route("/v1/imports/manifest", post(imports::import))
        .route("/v1/imports/mcp-http", post(imports::http))
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(api, auth::gate))
        .with_state(mcp)
}
pub(crate) async fn work<T, F>(mcp: Arc<Mcp>, job: F) -> Result<T, crate::errors::ApiError>
where
    T: Send + 'static,
    F: FnOnce(Arc<Mcp>) -> exoharness_mcp::Result<T> + Send + 'static,
{
    static LIMIT: OnceLock<Arc<Semaphore>> = OnceLock::new();
    let permit = LIMIT
        .get_or_init(|| Arc::new(Semaphore::new(4)))
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| McpError::Configuration)?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        job(mcp)
    })
    .await
    .map_err(|_| McpError::Configuration)?
    .map_err(Into::into)
}
