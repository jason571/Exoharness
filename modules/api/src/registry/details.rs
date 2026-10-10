use super::RegistryState;
use crate::errors::{ApiError, storage};
use axum::{
    Json,
    extract::{Path, State},
};
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

pub(super) async fn audit(State(state): State<RegistryState>) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |s| {
        s.registry_audit(&RegistryAccess::Administrator, 1000)
    })
    .await
    .map(|records| Json(json!(records)))
}

pub(super) async fn record(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |s| {
        Ok(json!(s.capability(&RegistryAccess::Administrator, &id)?))
    })
    .await
    .map(Json)
}
pub(super) async fn permissions(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |s| {
        Ok(json!(s.capability_permissions(
            &RegistryAccess::Administrator,
            &id
        )?))
    })
    .await
    .map(Json)
}
pub(super) async fn health(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |s| {
        s.capability(&RegistryAccess::Administrator, &id)?;
        Ok(s.capability_health(&RegistryAccess::Administrator, &id)?.map(|v| json!(v))
            .unwrap_or_else(|| json!({"capability_id":id,"status":"unknown","message":"尚未验证","checked_at":null})))
    }).await.map(Json)
}
pub(super) async fn secrets(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    crate::mcp::work(state.mcp, move |mcp| {
        let record = mcp.registry.0.capability(&RegistryAccess::Administrator, &id)?;
        Ok(json!(
            record
                .manifest
                .secret_refs
                .iter()
                .map(
                    |r| json!({"name":r.name,"provider":r.provider,"configured":r.provider=="local"&&mcp.secrets.configured(&r.key),"mutable":r.provider=="local"})
                )
                .collect::<Vec<_>>()
        ))
    })
    .await
    .map(Json)
}
pub(super) async fn runtime(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let view = super::preparation::inspect(state, id).await?;
    Ok(Json(
        json!({"applicable":view["scripts_present"].as_bool().unwrap_or(false),"ready":view["environment_ready"].as_bool().unwrap_or_else(||view["runtime_ready"].as_bool().unwrap_or(false)),"status":view["status"],"message":view["message"],"evidence":view["evidence"],"script_execution_ready":false}),
    ))
}
pub(super) async fn versions(
    State(state): State<RegistryState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    storage(state.registry.0.clone(), move |s| {
        Ok(json!(s.capability_versions(
            &RegistryAccess::Administrator,
            &id
        )?))
    })
    .await
    .map(Json)
}
