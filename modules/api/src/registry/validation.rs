use super::RegistryState;
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::Value;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Request {
    pub confirm: bool,
    #[serde(default)]
    pub actor: Option<String>,
}
pub(super) async fn validate(
    State(state): State<RegistryState>,
    Extension(principal): Extension<Principal>,
    Path(id): Path<String>,
    Json(request): Json<Request>,
) -> Result<Json<Value>, ApiError> {
    let _provenance = request.actor;
    if !request.confirm {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "请明确提交验证操作",
        ));
    }
    state
        .mcp
        .validate(RegistryAccess::Administrator, id, principal.id)
        .await
        .map(Json)
        .map_err(Into::into)
}
pub(super) async fn tools(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<Value>>, ApiError> {
    storage(state.registry.0.clone(), move |store| {
        store.discovered_tools(&RegistryAccess::Administrator, &id)
    })
    .await
    .map(Json)
}
