use super::{IdentityError, IdentityState, sessions::cookie_response, work};
use crate::auth;
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use exoharness_identity::{DEFAULT_SESSION_TTL_SECONDS, mailer::RegistrationEmailMessage};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Email {
    email: String,
}
fn origin(state: &IdentityState, headers: &HeaderMap) -> Result<(), IdentityError> {
    if !auth::request_origin_valid(headers, state.api.port) {
        return Err(IdentityError::new(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "请求来源与当前服务不匹配",
        ));
    }
    Ok(())
}
pub(crate) async fn code(
    State(state): State<IdentityState>,
    headers: HeaderMap,
    Json(input): Json<Email>,
) -> Result<(StatusCode, Json<Value>), IdentityError> {
    origin(&state, &headers)?;
    if !state.mailer.is_configured() {
        return Err(IdentityError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "email_not_configured",
            "邮箱验证服务尚未配置，暂时无法注册",
        ));
    }
    let mailer = state.mailer.clone();
    work(state.store, move |store| {
        if let Some(code) = store.issue_registration_code(&input.email)? {
            let template = store.registration_email_template()?;
            let (subject, body) = template.render(&code.code);
            if mailer
                .send_registration_code(
                    &code.email,
                    &code.code,
                    &RegistrationEmailMessage::new(subject, body),
                )
                .is_err()
            {
                store.discard_registration_code(&code.id)?;
                return Err(exoharness_identity::AuthError::EmailDelivery);
            }
            store.activate_registration_code(&code.id)?;
        }
        Ok(())
    })
    .await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(
            json!({"message":"若邮箱可以注册，验证码将发送到该邮箱","expires_in":600,"resend_after":60}),
        ),
    ))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Register {
    email: String,
    code: String,
    username: String,
    display_name: String,
    password: String,
}
pub(crate) async fn register(
    State(state): State<IdentityState>,
    headers: HeaderMap,
    Json(input): Json<Register>,
) -> Result<Response, IdentityError> {
    origin(&state, &headers)?;
    let session = work(state.store, move |store| {
        store.register_verified_user(
            &input.email,
            &input.code,
            &input.username,
            &input.display_name,
            &input.password,
            DEFAULT_SESSION_TTL_SECONDS,
        )
    })
    .await?;
    Ok(cookie_response(
        state.api.surface,
        &session.token,
        Some(session.account),
        "user",
        StatusCode::CREATED,
    ))
}
