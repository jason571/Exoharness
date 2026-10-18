// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ExternalState, assistant, work};
use crate::{auth::Principal, identity::IdentityError};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Response},
};
use exoharness_identity::external::AuthorizationRequest;
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) async fn page(
    State(state): State<ExternalState>,
    Query(request): Query<AuthorizationRequest>,
) -> Result<Response, IdentityError> {
    let copy = request.clone();
    let app = work(&state, move |s| s.validate_external_authorization(&copy)).await?;
    let profile = assistant(&state, &app.input.assistant_id).await?;
    let data=serde_json::to_string(&json!({"request":request,"application_name":app.input.name,"assistant_name":profile["display_name"]})).map_err(|_|IdentityError::unavailable())?
        .replace('<',"\\u003c").replace('>',"\\u003e").replace('&',"\\u0026");
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| IdentityError::unavailable())?;
    let nonce = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let html = include_str!("../../../../web/external-authorization.html")
        .replace("%DATA%", &data)
        .replace("%NONCE%", &nonce);
    let csp = format!(
        "default-src 'none'; script-src 'nonce-{nonce}'; style-src 'nonce-{nonce}'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'none'"
    );
    Ok((
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CONTENT_SECURITY_POLICY, csp.as_str()),
        ],
        Html(html),
    )
        .into_response())
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Approval {
    pub request: AuthorizationRequest,
    pub confirm: bool,
}
pub(super) async fn approve(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    headers: HeaderMap,
    Json(input): Json<Approval>,
) -> Result<Json<Value>, IdentityError> {
    // Consent must come from a logged-in user browser, never an application key.
    if headers.contains_key(header::AUTHORIZATION) || !input.confirm {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "user_consent_required",
            "Explicit browser user authorization is required",
        ));
    }
    let copy = input.request.clone();
    let app = work(&state, move |s| s.validate_external_authorization(&copy)).await?;
    assistant(&state, &app.input.assistant_id).await?;
    let session = crate::auth::cookie_id(&headers, state.api.surface)
        .ok_or_else(IdentityError::credentials)?
        .to_owned();
    let redirect = work(&state, move |s| {
        s.authorize_external_application_from_session(&principal.id, &session, &input.request)
    })
    .await?
    .ok_or_else(|| {
        IdentityError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_user_session",
            "Sign in again before authorizing this application",
        )
    })?;
    Ok(Json(json!({"redirect_uri":redirect})))
}
pub(super) async fn grants(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Value>, IdentityError> {
    Ok(Json(
        json!({"grants":work(&state,move|s|s.external_grants(&principal.id)).await?}),
    ))
}
pub(super) async fn revoke(
    State(state): State<ExternalState>,
    Extension(principal): Extension<Principal>,
    Path(grant): Path<String>,
) -> Result<StatusCode, IdentityError> {
    let revoked = work(&state, move |s| {
        s.revoke_external_grant(&principal.id, &grant)
    })
    .await?;
    if !revoked {
        return Err(IdentityError::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "User authorization was not found",
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}
