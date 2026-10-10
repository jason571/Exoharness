// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{IdentityError, IdentityState, work};
use crate::errors::storage;
use axum::{
    Json,
    extract::{Path, Query, State},
};
use exoharness_identity::{DEFAULT_AGENT_PROFILE_ID, UserAccount, UserCapabilityPolicy};
use serde::Deserialize;
use serde_json::{Value, json};
fn assistant_list(state: &IdentityState) -> Vec<Value> {
    state.assistants.list().into_iter().map(|profile| json!({"id":profile["id"],"name":profile["display_name"],"enabled":profile["enabled"],"published":profile.pointer("/access/published").cloned().unwrap_or(json!(false)),"is_default":profile["is_default"],"assignable":assignable(&profile)})).collect()
}
fn assignable(profile: &Value) -> bool {
    profile["enabled"] == true
        && (profile["is_default"] == true
            || profile.pointer("/access/published") == Some(&json!(true)))
}
fn projection(
    state: &IdentityState,
    account: UserAccount,
    assistant_id: String,
    policy: UserCapabilityPolicy,
) -> Value {
    let mut value = serde_json::to_value(account).expect("account serializable");
    let assistant = state.assistants.detail(&assistant_id);
    value["assistant_profile_id"] = json!(assistant_id);
    value["assistant_name"] = assistant
        .as_ref()
        .map(|a| a["display_name"].clone())
        .unwrap_or(Value::Null);
    value["assistant_available"] = json!(assistant.as_ref().is_some_and(assignable));
    value["capability_policy"] = json!(policy);
    value
}
#[derive(Deserialize, Default)]
pub(crate) struct Directory {
    search: Option<String>,
    after: Option<String>,
    limit: Option<usize>,
}
pub(crate) async fn list(
    State(state): State<IdentityState>,
    Query(query): Query<Directory>,
) -> Result<Json<Value>, IdentityError> {
    let limit = query.limit.unwrap_or(100);
    let search = query.search.unwrap_or_default();
    let (records, total) = work(state.store.clone(), move |store| {
        store.user_directory(&search, query.after.as_deref(), limit)
    })
    .await?;
    let ids = records
        .iter()
        .map(|(account, _)| account.id.clone())
        .collect::<Vec<_>>();
    let policies = work(state.store.clone(), {
        let ids = ids.clone();
        move |store| {
            ids.iter()
                .map(|id| store.user_capability_policy(id))
                .collect::<Result<Vec<_>, _>>()
        }
    })
    .await?;
    let activity = storage(state.api.execution.0.clone(), move |store| {
        store.user_execution_summaries(&ids)
    })
    .await
    .map_err(IdentityError::from)?;
    let next = if records.len() == limit {
        records.last().map(|(account, _)| account.id.clone())
    } else {
        None
    };
    let users=records.into_iter().zip(policies).map(|((account,id),policy)| {
        let stats=activity.get(&account.id).cloned().unwrap_or_else(||json!({"conversation_count":0,"run_count":0,"total_tokens":0,"last_activity_at":null}));
        let mut value=projection(&state,account,id,policy.unwrap_or_default());
        value.as_object_mut().expect("account object").extend(stats.as_object().expect("stats object").clone());
        value
    }).collect::<Vec<_>>();
    Ok(Json(
        json!({"users":users,"assistants":assistant_list(&state),"default_assistant_id":DEFAULT_AGENT_PROFILE_ID,"total":total,"next_cursor":next}),
    ))
}
pub(crate) async fn detail(
    State(state): State<IdentityState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, IdentityError> {
    let user_id = id.clone();
    let (account, assistant_id, policy) = work(state.store.clone(), move |store| {
        Ok((
            store.user_account(&user_id)?,
            store.user_agent_profile_id(&user_id)?,
            store.user_capability_policy(&user_id)?,
        ))
    })
    .await?;
    let account = account.ok_or_else(IdentityError::not_found)?;
    let mut value = projection(
        &state,
        account,
        assistant_id.unwrap_or_else(|| DEFAULT_AGENT_PROFILE_ID.into()),
        policy.unwrap_or_default(),
    );
    value["conversations"] = json!(
        storage(state.api.execution.0.clone(), move |store| store
            .user_conversations(&id, 100))
        .await
        .map_err(IdentityError::from)?
    );
    Ok(Json(value))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Assignment {
    agent_profile_id: String,
}
pub(crate) async fn assign(
    State(state): State<IdentityState>,
    Path(id): Path<String>,
    Json(input): Json<Assignment>,
) -> Result<Json<Value>, IdentityError> {
    let assigned = input.agent_profile_id.clone();
    let assistants = state.assistants;
    let assistant = work(state.store, move |store| {
        let lease =
            assistants
                .assignment(&input.agent_profile_id)
                .map_err(|error| match error {
                    exoharness_configuration::ConfigurationError::Conflict => {
                        exoharness_identity::AuthError::Conflict(
                            "助理配置正在更新，请刷新重试".into(),
                        )
                    }
                    _ => exoharness_identity::AuthError::Validation(
                        "只能选择已启用并发布的助理".into(),
                    ),
                })?;
        store.set_user_agent_profile_id(&id, &input.agent_profile_id)?;
        Ok(lease.profile().clone())
    })
    .await?;
    Ok(Json(
        json!({"assistant_profile_id":assigned,"assistant_name":assistant["display_name"],"assistant_available":true}),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Policy {
    policy: UserCapabilityPolicy,
}
pub(crate) async fn policy(
    State(state): State<IdentityState>,
    Path(id): Path<String>,
    Json(input): Json<Policy>,
) -> Result<Json<Value>, IdentityError> {
    let policy = work(state.store, move |store| {
        store.set_user_capability_policy(&id, input.policy)
    })
    .await?;
    Ok(Json(json!({"policy":policy})))
}
