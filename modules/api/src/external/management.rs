// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ExternalState, assistant, work};
use crate::auth::Principal;
use crate::identity::IdentityError;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
};
use exoharness_identity::external::{ApplicationInput, IssuedApplication};
use exoharness_secrets::SecretCipher;
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) async fn list(State(state): State<ExternalState>) -> Result<Json<Value>, IdentityError> {
    Ok(Json(
        json!({"applications":work(&state,|s|s.external_applications()).await?}),
    ))
}
pub(super) async fn grants(
    State(state): State<ExternalState>,
    Path(client): Path<String>,
) -> Result<Json<Value>, IdentityError> {
    Ok(Json(
        json!({"grants":work(&state,move|s|s.external_application_grants(&client)).await?}),
    ))
}
pub(super) async fn revoke_grant(
    State(state): State<ExternalState>,
    Path((client, grant)): Path<(String, String)>,
) -> Result<StatusCode, IdentityError> {
    if !work(&state, move |s| {
        s.revoke_external_application_grant(&client, &grant)
    })
    .await?
    {
        return Err(IdentityError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Application user authorization was not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
pub(super) async fn create(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    Json(input): Json<ApplicationInput>,
) -> Result<(StatusCode, Json<IssuedApplication>), IdentityError> {
    assistant(&state, &input.assistant_id).await?;
    let (secret, encrypted) = key(&state, &input.client_id).await?;
    let issued = work(&state, move |s| {
        let result = s.create_external_application_with_key(input, secret, encrypted)?;
        s.audit_external_key(&result.application.input.client_id, &principal.id, "create")?;
        Ok(result)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(issued)))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Update {
    pub application: ApplicationInput,
    pub expected_revision: i64,
    #[serde(default)]
    pub rotate_secret: bool,
}
pub(super) async fn update(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    Path(client): Path<String>,
    Json(input): Json<Update>,
) -> Result<Json<IssuedApplication>, IdentityError> {
    if client != input.application.client_id {
        return Err(IdentityError::new(
            StatusCode::BAD_REQUEST,
            "invalid_client",
            "Application ID does not match",
        ));
    }
    // Disabling remains possible when the assigned assistant was disabled.
    if input.application.enabled {
        assistant(&state, &input.application.assistant_id).await?;
    }
    let key = if input.rotate_secret {
        Some(key(&state, &client).await?)
    } else {
        None
    };
    Ok(Json(
        work(&state, move |s| {
            let result = s.update_external_application_with_key(
                input.application,
                input.expected_revision,
                key,
            )?;
            if input.rotate_secret {
                s.audit_external_key(&client, &principal.id, "rotate")?;
            }
            Ok(result)
        })
        .await?,
    ))
}
async fn key(state: &ExternalState, client: &str) -> Result<(String, String), IdentityError> {
    let root = state
        .api
        .credentials
        .external_key_directory
        .clone()
        .ok_or_else(IdentityError::unavailable)?;
    let client = client.to_owned();
    tokio::task::spawn_blocking(move || {
        let cipher = SecretCipher::open(&root).map_err(|_| IdentityError::unavailable())?;
        let secret = SecretCipher::generate_key().map_err(|_| IdentityError::unavailable())?;
        let encrypted = cipher
            .seal(&format!("external-application:{client}"), &secret)
            .map_err(|_| IdentityError::unavailable())?;
        Ok((secret, encrypted))
    })
    .await
    .map_err(|_| IdentityError::unavailable())?
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Delete {
    expected_revision: i64,
}
pub(super) async fn remove(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    Path(client): Path<String>,
    Json(input): Json<Delete>,
) -> Result<StatusCode, IdentityError> {
    if !work(&state, move |s| {
        s.delete_external_application(&client, input.expected_revision, &principal.id)
    })
    .await?
    {
        return Err(IdentityError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "Application was not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ViewKey {
    confirm: bool,
}
pub(super) async fn view_key(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    Path(client): Path<String>,
    Json(input): Json<ViewKey>,
) -> Result<Json<Value>, IdentityError> {
    if !input.confirm {
        return Err(IdentityError::new(
            StatusCode::BAD_REQUEST,
            "confirmation_required",
            "Confirm key reveal",
        ));
    }
    let root = state
        .api
        .credentials
        .external_key_directory
        .clone()
        .ok_or_else(IdentityError::unavailable)?;
    let copy = client.clone();
    let encrypted = work(&state, move |s| s.external_encrypted_key(&copy))
        .await?
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            IdentityError::new(
                StatusCode::NOT_FOUND,
                "key_unavailable",
                "Rotate this application's key to enable secure viewing",
            )
        })?;
    let context = format!("external-application:{client}");
    let secret = tokio::task::spawn_blocking(move || {
        SecretCipher::open(&root)?.open_secret(&context, &encrypted)
    })
    .await
    .map_err(|_| IdentityError::unavailable())?
    .map_err(|_| IdentityError::unavailable())?;
    let audit_client = client.clone();
    work(&state, move |s| {
        s.audit_external_key(&audit_client, &principal.id, "view")
    })
    .await?;
    Ok(Json(json!({"client_id":client,"client_secret":secret})))
}
