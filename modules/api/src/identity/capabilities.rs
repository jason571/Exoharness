// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Personal registry projection. Does not expose global/admin records or grant installation.
use super::{IdentityError, IdentityState, work};
use crate::{auth::Principal, errors::storage};
use axum::{Extension, Json, extract::State, http::StatusCode};
use exoharness_contracts::capability::CapabilityScope;
use exoharness_storage_postgres::RegistryAccess;
use serde_json::{Value, json};

pub(super) async fn catalog(
    State(state): State<IdentityState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, IdentityError> {
    let owner = principal.id.clone();
    let policy = work(state.store, move |store| {
        store.user_capability_policy(&owner)
    })
    .await?
    .ok_or_else(IdentityError::not_found)?;
    let registry = state.registry.ok_or_else(|| {
        IdentityError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "registry_unavailable",
            "个人能力登记服务暂时不可用，请稍后重试",
        )
    })?;
    let owner = principal.id;
    let records=storage(registry.0.clone(),move |store| {
        let records=store.capabilities(&RegistryAccess::User(owner.clone()),1000)?;
        Ok(records.into_iter().filter(|r| matches!(&r.manifest.scope,CapabilityScope::User{user_id} if user_id==&owner)).map(|r|json!({
            "id":r.id,"name":r.manifest.name,"kind":r.manifest.kind().as_str(),"revision":r.revision,"owner":true,
            "status":"unavailable","status_message":if r.enabled {"已登记；新版 Agent 执行尚未接入"} else {"已登记，尚未启用"},
            "next_action":null,"authorization_update_supported":false,"authorization_configured":false,
            "environment_secrets":[],"can_recheck":false
        })).collect::<Vec<_>>())
    }).await.map_err(|e|IdentityError::new(e.0,e.1,e.2))?;
    Ok(Json(
        json!({"capabilities":records,"policy":policy,"installation_ready":false}),
    ))
}

pub(super) async fn jobs(
    State(state): State<IdentityState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, IdentityError> {
    // The new framework has no install queue yet; report that fact, not a 404 or fake ready job.
    let owner = principal.id;
    work(state.store, move |store| {
        store.user_capability_policy(&owner)
    })
    .await?
    .ok_or_else(IdentityError::not_found)?;
    Ok(Json(json!({"jobs":[],"installation_ready":false})))
}
