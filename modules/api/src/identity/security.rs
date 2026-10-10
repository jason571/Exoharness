// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{IdentityError, IdentityState, accounts::Password, sessions::parse_role, work};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::Deserialize;
use serde_json::{Value, json};
pub(crate) async fn password(
    State(state): State<IdentityState>,
    Path(role): Path<String>,
    Json(input): Json<Password>,
) -> Result<Json<Value>, IdentityError> {
    let role = parse_role(&role)?;
    let updated = work(state.store, move |store| {
        store.rotate_role_password(&input.current_password, role, &input.new_password)
    })
    .await?;
    if !updated {
        return Err(IdentityError::credentials());
    }
    Ok(Json(
        json!({"updated":true,"role":role,"sessions_revoked":true}),
    ))
}
fn view(template: exoharness_identity::RegistrationEmailTemplate, configured: bool) -> Json<Value> {
    let mut value = serde_json::to_value(template).expect("template serializable");
    value["content_type"] = json!("text/plain");
    value["mailer_configured"] = json!(configured);
    value["placeholders"] = json!([{"token":"{{code}}","description":"注册验证码"},{"token":"{{expires_minutes}}","description":"有效分钟数"},{"token":"{{product_name}}","description":"产品名称"}]);
    Json(value)
}
pub(crate) async fn template(
    State(state): State<IdentityState>,
) -> Result<Json<Value>, IdentityError> {
    Ok(view(
        work(state.store, |store| store.registration_email_template()).await?,
        state.mailer.is_configured(),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Template {
    subject: String,
    body: String,
    revision: i64,
}
pub(crate) async fn update_template(
    State(state): State<IdentityState>,
    Json(input): Json<Template>,
) -> Result<Json<Value>, IdentityError> {
    Ok(view(
        work(state.store, move |store| {
            store.update_registration_email_template(&input.subject, &input.body, input.revision)
        })
        .await?,
        state.mailer.is_configured(),
    ))
}
