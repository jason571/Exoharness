// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Read-only administrator inspection without widening delegated access.
use super::{ExternalState, work};
use crate::errors::{ApiError, storage};
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use exoharness_storage_postgres::ExternalConversationQuery;
use serde::Deserialize;
use serde_json::{Value, json};

fn one() -> usize {
    1
}
fn ten() -> usize {
    10
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct List {
    client_id: Option<String>,
    #[serde(default)]
    q: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    device: String,
    #[serde(default = "one")]
    page: usize,
    #[serde(default = "ten")]
    limit: usize,
}
pub(super) async fn list(
    State(state): State<ExternalState>,
    Query(query): Query<List>,
) -> Result<Json<Value>, ApiError> {
    if query.page == 0
        || query.page > 1_000_000
        || !(1..=10).contains(&query.limit)
        || query.q.len() > 512
        || query.user.len() > 128
        || query.device.len() > 128
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "invalid_filter",
            "Invalid filter or page size; at most 10 conversations per page",
        ));
    }
    let (name, user) = (query.q.trim().to_owned(), query.user.trim().to_owned());
    let matching = name.clone();
    let (matching_users, user_ids) = work(&state, move |store| {
        let matching_users = if matching.is_empty() {
            vec![]
        } else {
            let mut ids = store.matching_user_ids(&matching)?;
            ids.extend(store.matching_external_user_ids(&matching)?);
            ids
        };
        let user_ids = if user.is_empty() {
            None
        } else {
            let mut ids = store.matching_user_ids(&user)?;
            ids.extend(store.matching_external_user_ids(&user)?);
            ids.push(user);
            Some(ids)
        };
        Ok((matching_users, user_ids))
    })
    .await?;
    let filter = ExternalConversationQuery {
        client_id: query.client_id.filter(|id| !id.is_empty()),
        query: name,
        matching_users,
        user_ids,
        device: query.device.trim().to_owned(),
        page: query.page,
        limit: query.limit,
    };
    let mut value = storage(state.api.execution.0.clone(), move |s| {
        s.management_external_conversations(filter)
    })
    .await?;
    let ids = value["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v["user_id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    let labels = work(&state, move |s| s.user_labels(&ids)).await?;
    for session in value["sessions"].as_array_mut().into_iter().flatten() {
        let id = session["user_id"].as_str().unwrap_or_default();
        session["user_label"] = json!(labels.get(id).map(String::as_str).unwrap_or(id));
    }
    let pairs = value["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| {
            Some((
                v["client_id"].as_str()?.to_owned(),
                v["user_id"].as_str()?.to_owned(),
            ))
        })
        .collect::<Vec<_>>();
    let projections = work(&state, move |s| {
        pairs
            .iter()
            .map(|(client, user)| s.external_user_projection(client, user))
            .collect::<Result<Vec<_>, _>>()
    })
    .await?;
    for (session, projection) in value["sessions"]
        .as_array_mut()
        .into_iter()
        .flatten()
        .zip(projections)
    {
        if let Some((subject, name)) = projection {
            session["external_user_id"] = json!(subject);
            session["user_label"] = json!(name);
        }
    }
    Ok(Json(value))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct History {
    before_ordinal: Option<i64>,
    #[serde(default = "ten")]
    turn_limit: usize,
}
pub(super) async fn detail(
    State(state): State<ExternalState>,
    Path((client, session)): Path<(String, String)>,
    Query(query): Query<History>,
) -> Result<Json<Value>, ApiError> {
    let mut value = storage(state.api.execution.0.clone(), move |s| {
        s.management_external_conversation(
            &client,
            &session,
            query.before_ordinal,
            query.turn_limit,
        )
    })
    .await?;
    let id = value["external"]["user_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let copy = id.clone();
    let labels = work(&state, move |s| s.user_labels(&[copy])).await?;
    value["external"]["user_label"] = json!(labels.get(&id).unwrap_or(&id));
    let client = value["external"]["client_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if let Some((subject, name)) =
        work(&state, move |s| s.external_user_projection(&client, &id)).await?
    {
        value["external"]["external_user_id"] = json!(subject);
        value["external"]["user_label"] = json!(name);
    }
    crate::public_chat::history(&mut value);
    Ok(Json(value))
}
