// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ExternalState, assistant, work};
use crate::{auth, identity::IdentityError};
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use exoharness_identity::external::{Application, IssuedTokens};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenRequest {
    client_id: String,
    client_secret: String,
    grant_type: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    external_user_id: Option<String>,
    display_name: Option<String>,
    device_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RevokeRequest {
    client_id: String,
    client_secret: String,
    token: String,
}
fn parse<T: serde::de::DeserializeOwned>(
    headers: &HeaderMap,
    body: &Bytes,
) -> Result<T, IdentityError> {
    let invalid = || {
        IdentityError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Invalid request body",
        )
    };
    match headers
        .get(header::CONTENT_TYPE)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
    {
        "application/x-www-form-urlencoded" => {
            serde_urlencoded::from_bytes(body).map_err(|_| invalid())
        }
        "application/json" => std::str::from_utf8(body)
            .map_err(|_| invalid())
            .and_then(|text| exoharness_contracts::strict_json::parse(text).map_err(|_| invalid()))
            .and_then(|v| serde_json::from_value(v).map_err(|_| invalid())),
        _ => Err(IdentityError::new(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "invalid_request",
            "Use JSON or form encoding",
        )),
    }
}
async fn client(
    state: &ExternalState,
    id: String,
    secret: String,
) -> Result<Application, IdentityError> {
    let app = work(state, move |s| {
        s.authenticate_external_application(&id, &secret)
    })
    .await?;
    if let Some(app) = app {
        return Ok(app);
    }
    work(state, move |s| {
        s.reserve_login_attempt("external-invalid-client")
    })
    .await?;
    Err(IdentityError::new(
        StatusCode::UNAUTHORIZED,
        "invalid_client",
        "Application authentication failed",
    ))
}
fn origin(state: &ExternalState, headers: &HeaderMap) -> Result<(), IdentityError> {
    if !auth::request_origin_valid(headers, state.api.port) {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "Request origin does not match",
        ));
    }
    Ok(())
}
pub(super) async fn exchange(
    State(state): State<ExternalState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, IdentityError> {
    origin(&state, &headers)?;
    let input: TokenRequest = parse(&headers, &body)?;
    let application_secret = input.client_secret.clone();
    if input.grant_type != "app_user"
        && (input.external_user_id.is_some()
            || input.display_name.is_some()
            || input.device_id.is_some())
    {
        return Err(IdentityError::new(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Backend identity fields require app_user",
        ));
    }
    let app = client(&state, input.client_id, input.client_secret).await?;
    assistant(&state, &app.input.assistant_id).await?;
    let required = |value: Option<String>| {
        value.ok_or_else(|| {
            IdentityError::new(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Missing grant parameter",
            )
        })
    };
    let result: Option<IssuedTokens> = match input.grant_type.as_str() {
        "app_user" => {
            if input.code.is_some()
                || input.redirect_uri.is_some()
                || input.code_verifier.is_some()
                || input.refresh_token.is_some()
            {
                return Err(IdentityError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "Unexpected browser authorization or refresh fields",
                ));
            }
            let subject = required(input.external_user_id)?;
            let (id, secret, name, device) = (
                app.input.client_id,
                application_secret,
                input.display_name,
                input.device_id,
            );
            work(&state, move |s| {
                s.external_backend_tokens(
                    &id,
                    &secret,
                    &subject,
                    name.as_deref(),
                    device.as_deref(),
                )
            })
            .await?
        }
        "authorization_code" => {
            if input.refresh_token.is_some() {
                return Err(IdentityError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "Unexpected refresh token",
                ));
            }
            let (code, redirect, verifier) = (
                required(input.code)?,
                required(input.redirect_uri)?,
                required(input.code_verifier)?,
            );
            work(&state, move |s| {
                s.exchange_external_code(&app, &code, &redirect, &verifier)
            })
            .await?
        }
        "refresh_token" => {
            if input.code.is_some() || input.redirect_uri.is_some() || input.code_verifier.is_some()
            {
                return Err(IdentityError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "Unexpected authorization code parameters",
                ));
            }
            let refresh = required(input.refresh_token)?;
            work(&state, move |s| s.refresh_external_token(&app, &refresh)).await?
        }
        _ => {
            return Err(IdentityError::new(
                StatusCode::BAD_REQUEST,
                "unsupported_grant_type",
                "Unsupported grant type",
            ));
        }
    };
    let result = result.ok_or_else(|| {
        IdentityError::new(
            StatusCode::BAD_REQUEST,
            "invalid_grant",
            "Authorization is invalid or expired; obtain fresh user authorization",
        )
    })?;
    Ok((
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
        ],
        Json(result),
    )
        .into_response())
}
pub(super) async fn revoke(
    State(state): State<ExternalState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, IdentityError> {
    origin(&state, &headers)?;
    let input: RevokeRequest = parse(&headers, &body)?;
    let app = client(&state, input.client_id, input.client_secret).await?;
    work(&state, move |s| s.revoke_external_token(&app, &input.token)).await?;
    Ok((
        StatusCode::NO_CONTENT,
        [(header::CACHE_CONTROL, "no-store")],
    )
        .into_response())
}
