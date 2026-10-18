// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ExternalState, assistant, work};
use crate::{
    auth::{self, Principal, Role},
    errors::{ApiError, storage},
    identity::IdentityError,
};
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub(super) async fn gate(
    State(state): State<ExternalState>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers().clone();
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    match authorize(&state, headers, method, path).await {
        Ok(grant) => {
            request.extensions_mut().insert(Principal {
                id: grant.user_id.clone(),
                role: Role::User,
            });
            request.extensions_mut().insert(grant);
            let mut response = next.run(request).await;
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                header::HeaderValue::from_static("no-store"),
            );
            response
        }
        Err(error) => error.into_response(),
    }
}
async fn authorize(
    state: &ExternalState,
    headers: axum::http::HeaderMap,
    method: Method,
    path: String,
) -> Result<exoharness_identity::external::Grant, IdentityError> {
    if !auth::request_origin_valid(&headers, state.api.port) {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "Request origin does not match",
        ));
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| {
            IdentityError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "A delegated access token is required",
            )
        })?
        .to_owned();
    let copy = token.clone();
    let access = work(state, move |s| s.validate_external_access(&copy)).await?;
    let grant = if let Some(grant) = access {
        grant
    } else if let (Some(client), Some(subject)) = (
        headers.get("x-exoharness-client-id"),
        headers.get("x-exoharness-user-id"),
    ) {
        let decode = |value: &axum::http::HeaderValue| -> Result<String, IdentityError> {
            let raw = value.to_str().map_err(|_| {
                IdentityError::new(
                    StatusCode::BAD_REQUEST,
                    "invalid_identity",
                    "Invalid identity header",
                )
            })?;
            let encoded = format!("value={raw}");
            Ok(url::form_urlencoded::parse(encoded.as_bytes())
                .next()
                .map(|(_, v)| v.into_owned())
                .unwrap_or_default())
        };
        let (client, subject) = (decode(client)?, decode(subject)?);
        let name = headers
            .get("x-exoharness-user-name")
            .map(decode)
            .transpose()?;
        let device = headers
            .get("x-exoharness-device-id")
            .map(decode)
            .transpose()?;
        work(state, move |s| {
            s.external_backend_grant(
                &client,
                &token,
                &subject,
                name.as_deref(),
                device.as_deref(),
            )
        })
        .await?
        .ok_or_else(|| {
            IdentityError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_client",
                "Application key is invalid or direct backend calls are disabled",
            )
        })?
    } else {
        return Err({
            IdentityError::new(
                StatusCode::UNAUTHORIZED,
                "invalid_token",
                "Access token is invalid or expired",
            )
        });
    };
    let path = path.strip_prefix("/v1/external/").unwrap_or("");
    let parts = path.split('/').collect::<Vec<_>>();
    let scope = match (&method, parts.as_slice()) {
        (&Method::GET, ["me"]) => "identity:read",
        (&Method::POST, ["sessions"]) => "sessions:write",
        (&Method::GET, ["sessions", _]) => "sessions:read",
        (&Method::POST, ["sessions", _, "turns"]) => "sessions:write",
        (&Method::PUT, ["sessions", _, "device"]) => "sessions:write",
        (&Method::GET, ["runs", _, "events"]) => "runs:read",
        (&Method::POST, ["runs", _, "cancel"]) => "runs:cancel",
        _ => {
            return Err(IdentityError::new(
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                "Endpoint is not available to external applications",
            ));
        }
    };
    if !grant.scopes.iter().any(|s| s == scope) {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "insufficient_scope",
            "The user grant does not include this permission",
        ));
    }
    assistant(state, &grant.assistant_id).await?;
    let session = match parts.as_slice() {
        ["sessions", sid, ..] => Some((*sid).to_owned()),
        ["runs", rid, ..] => {
            let (owner, run) = (grant.user_id.clone(), (*rid).to_owned());
            Some(
                storage(state.api.execution.0.clone(), move |s| {
                    s.owned_run(&owner, &run)
                })
                .await?
                .session_id,
            )
        }
        _ => None,
    };
    if let Some(session) = session {
        let (owner, client, assistant) = (
            grant.user_id.clone(),
            grant.client_id.clone(),
            grant.assistant_id.clone(),
        );
        let checked_session = session.clone();
        let authorized = storage(state.api.execution.0.clone(), move |s| {
            s.external_session_authorized(&owner, &client, &assistant, &checked_session)
        })
        .await?;
        if !authorized {
            return Err(ApiError(
                StatusCode::NOT_FOUND,
                "not_found",
                "Session was not found in this application account",
            )
            .into());
        }
        if grant.device_id.is_some()
            || (grant.require_device_id
                && method == Method::POST
                && matches!(parts.as_slice(), ["sessions", _, "turns"]))
        {
            let (owner, client) = (grant.user_id.clone(), grant.client_id.clone());
            let device = storage(state.api.execution.0.clone(), move |s| {
                s.external_session_device(&owner, &client, &session)
            })
            .await?;
            let attaching =
                method == Method::PUT && matches!(parts.as_slice(), ["sessions", _, "device"]);
            if device
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .is_none()
                && !attaching
            {
                return Err(IdentityError::new(
                    StatusCode::FORBIDDEN,
                    "device_binding_required",
                    "Device binding must be verified and recorded before starting an external conversation",
                ));
            }
            if device.is_some()
                && grant
                    .device_id
                    .as_ref()
                    .is_some_and(|expected| device.as_ref() != Some(expected))
            {
                return Err(IdentityError::new(
                    StatusCode::NOT_FOUND,
                    "not_found",
                    "Session was not found for this application user/device",
                ));
            }
        }
    }
    Ok(grant)
}
