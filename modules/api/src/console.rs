//! Serve the preserved pages verbatim; adapt only local login/session protocols.
use crate::{
    ApiState, Surface,
    auth::{self, Role},
    browser_sessions::cookie_name,
    errors::ApiError,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

pub(crate) fn routes() -> Router<ApiState> {
    Router::new()
        .route("/", get(home))
        .route("/exoharness-login", get(login_page))
        .route("/login", get(login_page))
        .route("/register", get(register_page))
        .route("/overview", get(management_page))
        .route("/services", get(management_page))
        .route("/imports", get(management_page))
        .route("/models", get(management_page))
        .route("/agents", get(management_page))
        .route("/knowledge", get(management_page))
        .route("/trace", get(management_page))
        .route("/audit", get(management_page))
        .route("/security", get(management_page))
        .route("/users", get(management_page))
        .route("/chat", get(portal_chat))
        .route("/settings", get(portal_settings))
        .route("/assets/{name}", get(asset))
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/status", get(auth_status))
}

async fn authenticated_page(
    state: &ApiState,
    headers: &HeaderMap,
    html: &'static str,
    portal: bool,
) -> Response {
    if (state.surface == Surface::Portal) != portal {
        return StatusCode::NOT_FOUND.into_response();
    }
    let session = match auth::browser(state, headers).await {
        Ok(session) => session,
        Err(error) => return error.into_response(),
    };
    if session.is_none_or(|session| !portal && session.principal.role != Role::Admin) {
        return Redirect::to(if portal {
            "/login"
        } else {
            "/exoharness-login"
        })
        .into_response();
    }
    Html(html).into_response()
}
async fn home(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    match state.surface {
        Surface::Management => {
            authenticated_page(
                &state,
                &headers,
                include_str!("../../../web/overview.html"),
                false,
            )
            .await
        }
        Surface::Portal => {
            authenticated_page(
                &state,
                &headers,
                include_str!("../../../web/portal-chat.html"),
                true,
            )
            .await
        }
    }
}
async fn login_page(State(state): State<ApiState>) -> Html<&'static str> {
    Html(match state.surface {
        Surface::Management => include_str!("../../../web/login.html"),
        Surface::Portal => include_str!("../../../web/portal-login.html"),
    })
}
async fn register_page(State(state): State<ApiState>) -> Response {
    if state.surface != Surface::Portal {
        return StatusCode::NOT_FOUND.into_response();
    }
    Html(include_str!("../../../web/portal-register.html")).into_response()
}
async fn management_page(
    State(state): State<ApiState>,
    headers: HeaderMap,
    uri: axum::http::Uri,
) -> Response {
    let html = match uri.path() {
        "/overview" => include_str!("../../../web/overview.html"),
        "/services" => include_str!("../../../web/index.html"),
        "/imports" => include_str!("../../../web/imports.html"),
        "/models" => include_str!("../../../web/models.html"),
        "/agents" => include_str!("../../../web/agents.html"),
        "/knowledge" => include_str!("../../../web/knowledge.html"),
        "/trace" => include_str!("../../../web/trace.html"),
        "/audit" => include_str!("../../../web/audit.html"),
        "/security" => include_str!("../../../web/security.html"),
        "/users" => include_str!("../../../web/users.html"),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    if uri.path() == "/services"
        && state.surface == Surface::Management
        && let Ok(Some(session)) = auth::browser(&state, &headers).await
        && session.principal.role == Role::ReadOnly
    {
        return Html(html).into_response();
    }
    authenticated_page(&state, &headers, html, false).await
}
async fn portal_chat(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    authenticated_page(
        &state,
        &headers,
        include_str!("../../../web/portal-chat.html"),
        true,
    )
    .await
}
async fn portal_settings(State(state): State<ApiState>, headers: HeaderMap) -> Response {
    authenticated_page(
        &state,
        &headers,
        include_str!("../../../web/portal-settings.html"),
        true,
    )
    .await
}
async fn asset(Path(name): Path<String>) -> Response {
    let (kind, content) = match name.as_str() {
        "i18n.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/i18n.js"),
        ),
        "styles.css" => (
            "text/css; charset=utf-8",
            include_str!("../../../web/styles.css"),
        ),
        "management.css" => (
            "text/css; charset=utf-8",
            include_str!("../../../web/management.css"),
        ),
        "management-theme.css" => (
            "text/css; charset=utf-8",
            include_str!("../../../web/management-theme.css"),
        ),
        "management-theme.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/management-theme.js"),
        ),
        "login.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/login.js"),
        ),
        "portal-login.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/portal-login.js"),
        ),
        "portal-register.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/portal-register.js"),
        ),
        "portal-auth.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/portal-auth.js"),
        ),
        "portal-theme.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/portal-theme.js"),
        ),
        "portal-settings.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/portal-settings.js"),
        ),
        "auth.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/auth.js"),
        ),
        "overview.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/overview.js"),
        ),
        "app.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/app.js"),
        ),
        "imports.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/imports.js"),
        ),
        "models.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/models.js"),
        ),
        "agents.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/agents.js"),
        ),
        "knowledge.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/knowledge.js"),
        ),
        "trace.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/trace.js"),
        ),
        "audit.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/audit.js"),
        ),
        "security.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/security.js"),
        ),
        "users.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/users.js"),
        ),
        "chat.js" => (
            "text/javascript; charset=utf-8",
            include_str!("../../../web/chat.js"),
        ),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    ([(header::CONTENT_TYPE, kind)], content).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    password: String,
    identifier: Option<String>,
}
async fn login(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<Response, crate::identity::IdentityError> {
    if state.credentials.identity.is_some() {
        return crate::identity::sessions::login(
            &state,
            &headers,
            input.password,
            input.identifier,
        )
        .await;
    }
    if !auth::request_origin_valid(&headers, state.port) {
        return Err(ApiError(
            StatusCode::FORBIDDEN,
            "invalid_origin",
            "请求来源与本机服务不匹配",
        )
        .into());
    }
    let principal = state
        .credentials
        .authenticate(&input.password)
        .ok_or(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码不正确",
        ))?;
    if (state.surface == Surface::Management && principal.role != Role::Admin)
        || (state.surface == Surface::Portal
            && input.identifier.as_deref() != Some(principal.id.as_str()))
    {
        return Err(ApiError(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码不正确",
        )
        .into());
    }
    let (id, _) = state
        .credentials
        .browsers
        .issue(principal.clone(), state.surface)
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "login_unavailable",
                "本机登录暂不可用",
            )
        })?;
    Ok((
        [
            (
                header::SET_COOKIE,
                format!(
                    "{}={id}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600",
                    cookie_name(state.surface)
                ),
            ),
            (header::CACHE_CONTROL, "no-store".to_owned()),
        ],
        Json(json!({"role":principal.role,"local_validation_only":true})),
    )
        .into_response())
}
pub(crate) async fn session(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    let session = auth::browser(&state, &headers).await?.ok_or(ApiError(
        StatusCode::UNAUTHORIZED,
        "authentication_required",
        "请先登录",
    ))?;
    let account = session.account.unwrap_or_else(||json!({"id":session.principal.id,"username":session.principal.id,"display_name":session.principal.id,"email":""}));
    Ok(Json(
        json!({"role":session.principal.role,"authentication_enabled":true,"csrf_token":session.csrf,"local_validation_only":state.credentials.identity.is_none(),"account":account}),
    ))
}
pub(crate) async fn logout(
    State(state): State<ApiState>,
    headers: HeaderMap,
) -> Result<Response, crate::identity::IdentityError> {
    if let Some(id) = auth::cookie_id(&headers, state.surface) {
        crate::identity::sessions::revoke(&state, id.to_owned()).await?;
        state.credentials.browsers.revoke(id, state.surface);
    }
    Ok((
        [(
            header::SET_COOKIE,
            format!(
                "{}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
                cookie_name(state.surface)
            ),
        )],
        Json(json!({"logged_out":true})),
    )
        .into_response())
}
async fn auth_status(
    State(state): State<ApiState>,
) -> Result<Json<Value>, crate::identity::IdentityError> {
    crate::identity::sessions::status(&state).await
}
