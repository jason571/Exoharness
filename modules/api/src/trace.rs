//! Management trace adapters. No mutation, provider calls or execution retries.
use crate::{
    ApiState, Surface,
    auth::{self, Principal, Role},
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    middleware,
    routing::get,
};
use serde::Deserialize;
use serde_json::Value;
pub(crate) fn router(state: ApiState) -> Router {
    if state.surface != Surface::Management {
        return Router::new();
    }
    Router::new()
        .route("/v1/trace/sessions", get(sessions))
        .route("/v1/trace/runs", get(runs))
        .route("/v1/trace/runs/{run}", get(detail))
        .route(
            "/v1/trace/runs/{run}/requests/{logical}/{attempt}",
            get(request),
        )
        .layer(middleware::from_fn_with_state(state.clone(), auth::gate))
        .with_state(state)
}
fn authorize(p: &Principal) -> Result<(), ApiError> {
    if p.role == Role::Admin {
        Ok(())
    } else {
        Err(ApiError(
            StatusCode::FORBIDDEN,
            "management_required",
            "此页面仅供管理端查看",
        ))
    }
}
#[derive(Deserialize, Default)]
struct Page {
    session_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
    event_after: Option<u64>,
    event_limit: Option<usize>,
    part: Option<String>,
}
async fn sessions(
    State(s): State<ApiState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<Page>,
) -> Result<Json<Vec<Value>>, ApiError> {
    authorize(&p)?;
    storage(s.execution.0.clone(), move |store| {
        store.trace_sessions(
            q.session_id.as_deref(),
            q.limit.unwrap_or(100),
            q.offset.unwrap_or(0),
        )
    })
    .await
    .map(Json)
}
async fn runs(
    State(s): State<ApiState>,
    Extension(p): Extension<Principal>,
    Query(q): Query<Page>,
) -> Result<Json<Vec<Value>>, ApiError> {
    authorize(&p)?;
    let id = q.session_id.ok_or(ApiError(
        StatusCode::BAD_REQUEST,
        "session_required",
        "请选择一个会话",
    ))?;
    storage(s.execution.0.clone(), move |store| {
        store.trace_runs(&id, q.limit.unwrap_or(100), q.offset.unwrap_or(0))
    })
    .await
    .map(Json)
}
async fn detail(
    State(s): State<ApiState>,
    Extension(p): Extension<Principal>,
    Path(run): Path<String>,
    Query(q): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    authorize(&p)?;
    storage(s.execution.0.clone(), move |store| {
        store.trace_run(
            &run,
            q.event_after.unwrap_or(0),
            q.event_limit.unwrap_or(256),
        )
    })
    .await
    .map(Json)
}
async fn request(
    State(s): State<ApiState>,
    Extension(p): Extension<Principal>,
    Path((run, logical, attempt)): Path<(String, String, i64)>,
    Query(q): Query<Page>,
) -> Result<Json<Value>, ApiError> {
    authorize(&p)?;
    storage(s.execution.0.clone(), move |store| {
        store.trace_request(&run, &logical, attempt, q.part.as_deref())
    })
    .await
    .map(Json)
}
