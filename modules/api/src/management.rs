// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Management projections only; no scheduler or model work is fabricated.
use crate::{
    ApiState, Surface, auth,
    errors::{ApiError, storage},
};
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    middleware,
    routing::get,
};
use exoharness_configuration::Assistants;
use exoharness_storage_postgres::PostgresStore;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

#[derive(Clone)]
struct ManagementState {
    store: PostgresStore,
    assistants: Arc<Assistants>,
    credentials: Arc<auth::Credentials>,
}
pub fn management_router(auth_state: ApiState, assistants: Arc<Assistants>) -> Router {
    if auth_state.surface != Surface::Management {
        return Router::new();
    }
    let trace = crate::trace::router(auth_state.clone());
    let state = ManagementState {
        store: auth_state.execution.0.clone(),
        assistants,
        credentials: auth_state.credentials.clone(),
    };
    Router::new()
        .route("/v1/management/overview", get(overview))
        .layer(middleware::from_fn_with_state(auth_state, auth::gate))
        .with_state(state)
        .merge(trace)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OverviewQuery {
    #[serde(default = "default_days")]
    days: u64,
    user: Option<String>,
}
fn default_days() -> u64 {
    7
}
async fn overview(
    State(state): State<ManagementState>,
    Query(query): Query<OverviewQuery>,
) -> Result<Json<Value>, ApiError> {
    if !(1..=365).contains(&query.days)
        || query
            .user
            .as_ref()
            .is_some_and(|value| value.chars().count() > 128)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_filter",
            "请选择有效的统计范围和用户名称",
        ));
    }
    let user = query.user.map(|value| value.trim().to_owned());
    let matched_users = if let (Some(identity), Some(name)) = (
        state.credentials.identity.clone(),
        user.as_ref().filter(|name| !name.is_empty()),
    ) {
        let name = name.clone();
        let mut ids =
            crate::identity::work(identity, move |store| store.matching_user_ids(&name)).await?;
        let needle = user.as_ref().unwrap().to_lowercase();
        ids.extend(
            state
                .credentials
                .principal_ids()
                .into_iter()
                .filter(|id| id.to_lowercase().contains(&needle)),
        );
        // Exact owner IDs remain usable for historical/deleted accounts. Do
        // not substring-match opaque IDs when the input is an account name.
        ids.push(user.as_ref().unwrap().clone());
        ids.sort();
        ids.dedup();
        Some(ids)
    } else {
        None
    };
    let scheduler = storage(state.store.clone(), |store| store.scheduler_status()).await?;
    let mut value = storage(state.store, move |store| match matched_users {
        Some(ids) => store.management_overview_for_users(query.days, &ids),
        None => store.management_overview(query.days, user.as_deref()),
    })
    .await?;
    let ids = value["groups"]["users"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|row| row["id"].as_str())
        .chain(
            value["activity"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| row["user_id"].as_str()),
        )
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    value["user_labels"] = if let Some(identity) = state.credentials.identity.clone() {
        json!(crate::identity::work(identity, move |store| store.user_labels(&ids)).await?)
    } else {
        json!({})
    };
    value["assistants"] = json!(
        state
            .assistants
            .list()
            .into_iter()
            .map(|profile| json!({
        "id":profile["id"],"name":profile["display_name"],"enabled":profile["enabled"],
        "skill_ids":profile["skill_ids"],"access":profile["access"]}))
            .collect::<Vec<_>>()
    );
    value["scheduler"] = scheduler;
    Ok(Json(value))
}
