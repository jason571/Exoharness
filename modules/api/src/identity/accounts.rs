// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{IdentityError, IdentityState, work};
use crate::auth::Principal;
use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
pub(crate) async fn account(
    State(state): State<IdentityState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, IdentityError> {
    let account = work(state.store, move |store| store.user_account(&principal.id))
        .await?
        .ok_or_else(IdentityError::not_found)?;
    Ok(Json(json!({"account":account})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    display_name: String,
}
pub(crate) async fn profile(
    State(state): State<IdentityState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<Profile>,
) -> Result<Json<Value>, IdentityError> {
    let account = work(state.store, move |store| {
        store.update_user_profile(&principal.id, &input.display_name)
    })
    .await?;
    Ok(Json(json!({"account":account})))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Password {
    pub current_password: String,
    pub new_password: String,
}
pub(crate) async fn password(
    State(state): State<IdentityState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<Password>,
) -> Result<Json<Value>, IdentityError> {
    let updated = work(state.store, move |store| {
        store.update_user_password(&principal.id, &input.current_password, &input.new_password)
    })
    .await?;
    if !updated {
        return Err(IdentityError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_password",
            "当前密码不正确，请重新输入",
        ));
    }
    Ok(Json(json!({"updated":true,"sessions_revoked":true})))
}
