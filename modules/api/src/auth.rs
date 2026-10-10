// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::errors::ApiError;
use crate::{
    ApiState, Surface,
    browser_sessions::{BrowserSession, BrowserSessions, cookie_name},
};
use axum::{
    Json,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    #[serde(rename = "chat")]
    ReadOnly,
    User,
}

#[derive(Debug, Clone, Serialize)]
pub struct Principal {
    pub id: String,
    pub role: Role,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "snake_case")]
enum CredentialKind {
    #[default]
    ApiKey,
    LocalPassword,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    id: String,
    role: Role,
    token: String,
    #[serde(default)]
    kind: CredentialKind,
}

pub struct Credentials {
    entries: Vec<(Principal, [u8; 32], bool)>,
    pub(crate) identity: Option<std::sync::Arc<exoharness_identity::AuthStore>>,
    pub(crate) registration_available: bool,
    pub(crate) browsers: BrowserSessions,
}
impl Credentials {
    pub fn from_json(data: &str) -> Result<Self, &'static str> {
        let records: Vec<Credential> =
            serde_json::from_str(data).map_err(|_| "invalid credential file")?;
        if records.is_empty() {
            return Err("credential file is empty");
        }
        let mut ids = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        let mut out = Vec::new();
        for record in records {
            if record.id.is_empty()
                || record.id.len() > 128
                || record.id.chars().any(char::is_control)
                || !ids.insert(record.id.clone())
            {
                return Err("invalid or duplicate principal");
            }
            match record.kind {
                CredentialKind::ApiKey if record.token.len() < 32 => {
                    return Err("API key must contain at least 32 bytes");
                }
                CredentialKind::LocalPassword
                    if record.role != Role::Admin || record.token.len() < 10 =>
                {
                    return Err("local administrator password must contain at least 10 bytes");
                }
                _ => {}
            }
            let hash: [u8; 32] = Sha256::digest(record.token.as_bytes()).into();
            if !hashes.insert(hash) {
                return Err("credentials must be distinct");
            }
            out.push((
                Principal {
                    id: record.id,
                    role: record.role,
                },
                hash,
                matches!(record.kind, CredentialKind::LocalPassword),
            ));
        }
        Ok(Self {
            entries: out,
            identity: None,
            registration_available: false,
            browsers: BrowserSessions::default(),
        })
    }
    pub fn authenticate(&self, token: &str) -> Option<Principal> {
        let hash: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        self.entries
            .iter()
            .find(|(_, expected, local_password)| {
                !(self.identity.is_some() && *local_password)
                    && expected
                        .iter()
                        .zip(hash)
                        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
                        == 0
            })
            .map(|(principal, _, _)| principal.clone())
    }
    pub fn with_identity(
        data: &str,
        store: std::sync::Arc<exoharness_identity::AuthStore>,
        registration_available: bool,
    ) -> Result<Self, String> {
        let mut credentials = Self::from_json(data).map_err(str::to_owned)?;
        let records: Vec<Credential> =
            serde_json::from_str(data).map_err(|_| "invalid credential file")?;
        for record in records {
            if record.role == Role::Admin {
                store
                    .bootstrap_password(exoharness_identity::AuthRole::Admin, &record.token)
                    .map_err(|_| "administrator password initialization failed")?;
            }
        }
        credentials.identity = Some(store);
        credentials.registration_available = registration_available;
        Ok(credentials)
    }
    pub(crate) async fn resolve(&self, token: &str) -> Result<Option<Principal>, ApiError> {
        if let Some(principal) = self.authenticate(token) {
            return Ok(Some(principal));
        }
        let Some(store) = self.identity.clone() else {
            return Ok(None);
        };
        let token = token.to_owned();
        let role = crate::identity::work(store, move |store| store.authenticate(&token)).await?;
        Ok(role.map(|role| Principal {
            id: "management".into(),
            role: match role {
                exoharness_identity::AuthRole::Admin => Role::Admin,
                exoharness_identity::AuthRole::Chat => Role::ReadOnly,
            },
        }))
    }
}

pub(crate) fn cookie_id(headers: &axum::http::HeaderMap, surface: Surface) -> Option<&str> {
    let prefix = format!("{}=", cookie_name(surface));
    let mut matches = headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|part| part.trim().strip_prefix(&prefix));
    let id = matches.next()?;
    if matches.next().is_some()
        || !matches!(id.len(), 43 | 64)
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return None;
    }
    Some(id)
}
pub(crate) async fn browser(
    state: &ApiState,
    headers: &axum::http::HeaderMap,
) -> Result<Option<BrowserSession>, ApiError> {
    let Some(id) = cookie_id(headers, state.surface) else {
        return Ok(None);
    };
    if let Some(store) = state.credentials.identity.clone() {
        let id = id.to_owned();
        let surface = state.surface;
        return crate::identity::work(store, move |store| {
            let csrf = exoharness_identity::csrf_token(&id);
            Ok(match surface {
                Surface::Management => store.validate_session(&id)?.map(|session| {
                    BrowserSession::persisted(
                        Principal {
                            id: "management".into(),
                            role: match session.role {
                                exoharness_identity::AuthRole::Admin => Role::Admin,
                                exoharness_identity::AuthRole::Chat => Role::ReadOnly,
                            },
                        },
                        csrf,
                        surface,
                        None,
                    )
                }),
                Surface::Portal => store.validate_user_session(&id)?.map(|session| {
                    BrowserSession::persisted(
                        Principal {
                            id: session.account.id.clone(),
                            role: Role::User,
                        },
                        csrf,
                        surface,
                        Some(serde_json::to_value(session.account).expect("account serializable")),
                    )
                }),
            })
        })
        .await
        .map_err(Into::into);
    }
    Ok(state.credentials.browsers.get(id, state.surface))
}
pub(crate) fn request_origin_valid(headers: &axum::http::HeaderMap, port: u16) -> bool {
    let hosts = [format!("127.0.0.1:{port}"), format!("localhost:{port}")];
    headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| hosts.iter().any(|allowed| allowed == host))
        && headers.get(header::ORIGIN).is_none_or(|value| {
            value
                .to_str()
                .ok()
                .is_some_and(|origin| hosts.iter().any(|host| origin == format!("http://{host}")))
        })
}

pub async fn gate(State(state): State<ApiState>, mut request: Request, next: Next) -> Response {
    let deny = |status: StatusCode, code: &str, message: &str| {
        (
            status,
            Json(serde_json::json!({"error":{"code":code,"message":message},"resumable":false})),
        )
            .into_response()
    };
    let hosts = [
        format!("127.0.0.1:{}", state.port),
        format!("localhost:{}", state.port),
    ];
    if !request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| hosts.iter().any(|allowed| h == allowed))
    {
        return deny(
            StatusCode::BAD_REQUEST,
            "invalid_host",
            "请求地址与当前本机服务不匹配",
        );
    }
    if let Some(origin) = request.headers().get(header::ORIGIN)
        && !origin
            .to_str()
            .ok()
            .is_some_and(|origin| hosts.iter().any(|host| origin == format!("http://{host}")))
    {
        return deny(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "请求来源与当前服务不匹配",
        );
    }
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));
    let bearer = if let Some(token) = token {
        match state.credentials.resolve(token).await {
            Ok(value) => value,
            Err(error) => return error.into_response(),
        }
    } else {
        None
    };
    let browser = if !request.headers().contains_key(header::AUTHORIZATION) {
        match browser(&state, request.headers()).await {
            Ok(value) => value,
            Err(error) => return error.into_response(),
        }
    } else {
        None
    };
    let principal = bearer.or_else(|| browser.as_ref().map(|session| session.principal.clone()));
    let Some(principal) = principal else {
        return deny(
            StatusCode::UNAUTHORIZED,
            "authentication_required",
            "请输入本机验证访问密钥",
        );
    };
    if let Some(session) = browser
        && !matches!(
            *request.method(),
            axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
        )
        && request
            .headers()
            .get("x-exoharness-csrf")
            .and_then(|value| value.to_str().ok())
            != Some(session.csrf.as_str())
    {
        return deny(
            StatusCode::FORBIDDEN,
            "csrf_required",
            "请求验证已失效，请刷新后重试",
        );
    }
    let readonly_allowed = principal.role == Role::ReadOnly
        && ((request.method() == axum::http::Method::GET
            && (matches!(
                request.uri().path(),
                "/v1/auth/session"
                    | "/v1/capabilities"
                    | "/v1/capabilities/overview"
                    | "/v1/assistant-capabilities"
            ) || readonly_capability_path(request.uri().path())))
            || (request.method() == axum::http::Method::POST
                && request.uri().path() == "/v1/auth/logout"));
    if state.surface == Surface::Management && principal.role != Role::Admin && !readonly_allowed {
        return deny(
            StatusCode::FORBIDDEN,
            "administrator_required",
            "管理端仅允许管理员访问",
        );
    }
    if state.surface == Surface::Portal && principal.role != Role::User {
        return deny(
            StatusCode::FORBIDDEN,
            "user_session_required",
            "请使用注册账号登录用户端",
        );
    }
    request.extensions_mut().insert(principal);
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store"),
    );
    response
}

// Explicit read-only allowlist: never accept arbitrary admin paths by prefix.
fn readonly_capability_path(path: &str) -> bool {
    let Some(tail) = path.strip_prefix("/v1/capabilities/") else {
        return false;
    };
    let parts = tail.split('/').collect::<Vec<_>>();
    matches!(parts.as_slice(), [id] if !id.is_empty())
        || matches!(parts.as_slice(), [id, endpoint] if !id.is_empty() && matches!(*endpoint,
            "permissions"|"health"|"secrets"|"runtime"|"versions"|"tools"|"skill-instructions"|"skill-files"|"preparation"))
}
