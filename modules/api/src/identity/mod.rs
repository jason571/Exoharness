// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Identity HTTP adapters are separate from execution and configuration services.
mod accounts;
mod capabilities;
mod errors;
mod password_reset;
mod registration;
mod security;
pub(crate) mod sessions;
mod users;
use crate::{ApiState, Surface, auth, errors::ApiError};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    middleware,
    routing::{get, post, put},
};
pub(crate) use errors::IdentityError;
use exoharness_configuration::Assistants;
use exoharness_identity::{AuthError, AuthStore, mailer::RegistrationMailer};
use std::sync::{Arc, LazyLock};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub(crate) struct IdentityState {
    pub store: Arc<AuthStore>,
    pub mailer: Arc<dyn RegistrationMailer>,
    pub assistants: Arc<Assistants>,
    pub api: ApiState,
    pub registry: Option<Arc<exoharness_registry::Registry>>,
    pub password_reset_base: Option<url::Url>,
}

pub fn identity_router(
    api: ApiState,
    store: Arc<AuthStore>,
    mailer: Arc<dyn RegistrationMailer>,
    assistants: Arc<Assistants>,
) -> Router {
    identity_router_with_registry(api, store, mailer, assistants, None)
}
pub fn identity_router_with_registry(
    api: ApiState,
    store: Arc<AuthStore>,
    mailer: Arc<dyn RegistrationMailer>,
    assistants: Arc<Assistants>,
    registry: Option<Arc<exoharness_registry::Registry>>,
) -> Router {
    let public_url = format!("http://127.0.0.1:{}", api.port);
    identity_router_with_public_url(api, store, mailer, assistants, registry, &public_url)
}
pub fn identity_router_with_public_url(
    api: ApiState,
    store: Arc<AuthStore>,
    mailer: Arc<dyn RegistrationMailer>,
    assistants: Arc<Assistants>,
    registry: Option<Arc<exoharness_registry::Registry>>,
    public_url: &str,
) -> Router {
    let external_management = crate::external_management_router(api.clone(), assistants.clone());
    let state = IdentityState {
        store,
        mailer,
        assistants,
        api: api.clone(),
        registry,
        password_reset_base: password_reset::public_base(public_url),
    };
    let protected = match api.surface {
        Surface::Management => Router::new()
            .route("/v1/management/users", get(users::list))
            .route("/v1/management/users/{id}", get(users::detail))
            .route("/v1/management/users/{id}/assistant", put(users::assign))
            .route(
                "/v1/management/users/{id}/capability-policy",
                put(users::policy),
            )
            .route("/v1/auth/passwords/{role}", put(security::password))
            .route(
                "/v1/auth/registration-email",
                get(security::template).put(security::update_template),
            ),
        Surface::Portal => Router::new()
            .route("/v1/user-capabilities", get(capabilities::catalog))
            .route(
                "/v1/user-capabilities/install-jobs",
                get(capabilities::jobs),
            )
            .route("/v1/account", get(accounts::account))
            .route("/v1/account/profile", put(accounts::profile))
            .route("/v1/account/password", put(accounts::password)),
    }
    .layer(middleware::from_fn_with_state(api.clone(), auth::gate));
    let public = if api.surface == Surface::Portal {
        Router::new()
            .route("/v1/auth/registration-code", post(registration::code))
            .route("/v1/auth/register", post(registration::register))
            .route("/v1/auth/password-reset", post(password_reset::request))
            .route(
                "/v1/auth/password-reset/validate",
                post(password_reset::validate),
            )
            .route(
                "/v1/auth/password-reset/complete",
                post(password_reset::complete),
            )
    } else {
        Router::new()
    };
    protected
        .merge(public)
        .layer(DefaultBodyLimit::max(32 * 1024))
        .with_state(state)
        .merge(external_management)
}

/// Bounded hashing and database workers; never block an async request worker.
pub(crate) async fn work<T, F>(store: Arc<AuthStore>, f: F) -> Result<T, IdentityError>
where
    T: Send + 'static,
    F: FnOnce(&AuthStore) -> Result<T, AuthError> + Send + 'static,
{
    static WORKERS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));
    static ADMISSION: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(64)));
    let admission = ADMISSION
        .clone()
        .try_acquire_owned()
        .map_err(|_| IdentityError::busy())?;
    let permit = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        WORKERS.clone().acquire_owned(),
    )
    .await
    .map_err(|_| IdentityError::busy())?
    .map_err(|_| IdentityError::busy())?;
    tokio::task::spawn_blocking(move || {
        let _admission = admission;
        let _permit = permit;
        f(&store)
    })
    .await
    .map_err(|_| IdentityError::unavailable())?
    .map_err(Into::into)
}
#[cfg(test)]
#[path = "../../tests/unit/identity_workers.rs"]
mod worker_tests;

impl From<IdentityError> for ApiError {
    fn from(error: IdentityError) -> Self {
        Self(
            error.status,
            error.code,
            match error.code {
                "rate_limited" => "请求过于频繁，请稍后再试",
                "identity_busy" => "身份服务繁忙，请稍后再试",
                "invalid_input" => "账号参数不符合要求",
                "state_conflict" => "记录已更新，请刷新后重试",
                _ => "身份服务暂时不可用，请稍后再试",
            },
        )
    }
}
