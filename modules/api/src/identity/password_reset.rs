// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{IdentityError, IdentityState, registration::origin, work};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use exoharness_identity::{
    AuthError, PASSWORD_RESET_RESEND_SECONDS, PASSWORD_RESET_TTL_SECONDS,
    mailer::RegistrationEmailMessage,
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(super) fn public_base(value: &str) -> Option<url::Url> {
    let parsed = url::Url::parse(value).ok()?;
    let loopback = parsed.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
    });
    ((parsed.scheme() == "https" || parsed.scheme() == "http" && loopback)
        && parsed.host_str().is_some()
        && parsed.username().is_empty()
        && parsed.password().is_none()
        && parsed.query().is_none()
        && parsed.fragment().is_none()
        && parsed.path() == "/")
        .then_some(parsed)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Email {
    email: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Token {
    token: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Complete {
    token: String,
    new_password: String,
}

pub(super) async fn request(
    State(state): State<IdentityState>,
    headers: HeaderMap,
    Json(input): Json<Email>,
) -> Result<(StatusCode, Json<Value>), IdentityError> {
    origin(&state, &headers)?;
    let Some(mut link) = state
        .password_reset_base
        .as_ref()
        .and_then(|base| base.join("reset-password").ok())
    else {
        return Err(IdentityError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "password_reset_not_configured",
            "找回密码服务尚未配置，请联系管理员",
        ));
    };
    if !state.mailer.is_configured() {
        return Err(IdentityError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_not_configured",
            "邮件服务尚未配置，请联系管理员",
        ));
    }
    let mailer = state.mailer.clone();
    let sent=work(state.store,move|store|{
        let Some(reset)=store.issue_password_reset(&input.email)? else {return Ok(false);};
        // The fragment is read only by the browser; it never enters access logs
        // or the page's HTTP request/referrer. The base is operator configuration.
        link.set_fragment(Some(&format!("token={}",reset.token)));
        let message=RegistrationEmailMessage::new("ExoHarness 重置密码".into(),format!("打开以下链接设置新密码：\n\n{link}\n\n链接有效期为 {} 分钟，且只能使用一次。如非本人操作，请忽略这封邮件。",PASSWORD_RESET_TTL_SECONDS/60));
        if mailer.send_password_reset(&reset.email,link.as_str(),&message).is_err(){
            store.discard_password_reset(&reset.id)?;return Err(AuthError::EmailDelivery);
        }
        if let Err(error)=store.activate_password_reset(&reset.id) {
            store.discard_password_reset(&reset.id)?;return Err(error);
        }
        Ok(true)
    }).await.map_err(|error|if error.code=="email_delivery_failed" {
        IdentityError::new(StatusCode::SERVICE_UNAVAILABLE,"email_delivery_failed","重置邮件未能发送，请稍后重试")
    }else{error})?;
    if !sent {
        return Err(IdentityError::new(
            StatusCode::NOT_FOUND,
            "email_not_registered",
            "该邮箱尚未注册，请先注册",
        ));
    }
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"message":"重置链接已发送，请在20分钟内打开邮件设置新密码","expires_in":PASSWORD_RESET_TTL_SECONDS,"resend_after":PASSWORD_RESET_RESEND_SECONDS}),
        ),
    ))
}
pub(super) async fn validate(
    State(state): State<IdentityState>,
    headers: HeaderMap,
    Json(input): Json<Token>,
) -> Result<Json<Value>, IdentityError> {
    origin(&state, &headers)?;
    let expires = work(state.store, move |store| {
        store.password_reset_valid_for(&input.token)
    })
    .await?;
    Ok(Json(json!({"valid":true,"expires_in":expires})))
}
pub(super) async fn complete(
    State(state): State<IdentityState>,
    headers: HeaderMap,
    Json(input): Json<Complete>,
) -> Result<Json<Value>, IdentityError> {
    origin(&state, &headers)?;
    work(state.store, move |store| {
        store.complete_password_reset(&input.token, &input.new_password)
    })
    .await?;
    Ok(Json(json!({"message":"密码已重置，请使用新密码登录"})))
}
