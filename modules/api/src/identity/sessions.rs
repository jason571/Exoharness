// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{IdentityError, work};
use crate::{ApiState, Surface, auth, browser_sessions::cookie_name};
use axum::{
    Json,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use exoharness_identity::{AuthRole, DEFAULT_SESSION_TTL_SECONDS};
use serde_json::json;

pub(crate) fn cookie_response(
    surface: Surface,
    token: &str,
    account: Option<exoharness_identity::UserAccount>,
    role: &str,
    status: StatusCode,
) -> Response {
    (status,[(header::SET_COOKIE,format!("{}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={DEFAULT_SESSION_TTL_SECONDS}",cookie_name(surface))),(header::CACHE_CONTROL,"no-store".into())],Json(json!({"role":role,"account":account,"local_validation_only":false}))).into_response()
}
pub(crate) async fn login(
    state: &ApiState,
    headers: &HeaderMap,
    password: String,
    identifier: Option<String>,
) -> Result<Response, IdentityError> {
    if !auth::request_origin_valid(headers, state.port) {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "请求来源与当前服务不匹配",
        ));
    }
    let store = state
        .credentials
        .identity
        .clone()
        .ok_or_else(IdentityError::unavailable)?;
    match state.surface {
        Surface::Management => {
            let session = work(store, move |store| {
                store.reserve_login_attempt("management-login")?;
                store.authenticate_session(&password, DEFAULT_SESSION_TTL_SECONDS)
            })
            .await?
            .ok_or_else(IdentityError::credentials)?;
            Ok(cookie_response(
                state.surface,
                &session.token,
                None,
                session.role.as_str(),
                StatusCode::OK,
            ))
        }
        Surface::Portal => {
            let identifier = identifier.ok_or_else(IdentityError::credentials)?;
            let session = work(store, move |store| {
                store.reserve_login_attempt(&format!(
                    "portal-login:{}",
                    identifier.trim().to_ascii_lowercase()
                ))?;
                store.authenticate_user(&identifier, &password, DEFAULT_SESSION_TTL_SECONDS)
            })
            .await?
            .ok_or_else(IdentityError::credentials)?;
            Ok(cookie_response(
                state.surface,
                &session.token,
                Some(session.account),
                "user",
                StatusCode::OK,
            ))
        }
    }
}
pub(crate) async fn status(state: &ApiState) -> Result<Json<serde_json::Value>, IdentityError> {
    if let Some(store) = state.credentials.identity.clone() {
        let configuration = work(store, |store| store.configuration()).await?;
        Ok(Json(
            json!({"authentication_enabled":true,"admin_configured":configuration.admin_configured,"chat_configured":configuration.chat_configured,"registration_available":state.credentials.registration_available,"local_validation_only":false}),
        ))
    } else {
        Ok(Json(
            json!({"authentication_enabled":true,"registration_available":false,"local_validation_only":true}),
        ))
    }
}
pub(crate) async fn revoke(state: &ApiState, token: String) -> Result<(), IdentityError> {
    let Some(store) = state.credentials.identity.clone() else {
        return Ok(());
    };
    let surface = state.surface;
    work(store, move |store| match surface {
        Surface::Management => {
            if let Some(session) = store.validate_session(&token)? {
                store.delete_session_digest(&session.token_digest)?;
            }
            Ok(())
        }
        Surface::Portal => {
            if let Some(session) = store.validate_user_session(&token)? {
                store.delete_user_session_digest(&session.token_digest)?;
            }
            Ok(())
        }
    })
    .await
}

pub(crate) fn parse_role(value: &str) -> Result<AuthRole, IdentityError> {
    AuthRole::parse(value).ok_or_else(|| {
        IdentityError::new(
            StatusCode::BAD_REQUEST,
            "invalid_role",
            "只能修改管理员或只读用户的密码",
        )
    })
}
