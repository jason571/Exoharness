use axum::{
    Json,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use exoharness_identity::AuthError;
use serde_json::json;
pub(crate) struct IdentityError {
    pub status: StatusCode,
    pub code: &'static str,
    message: String,
    retry_after: Option<i64>,
}
impl IdentityError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }
    pub fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "identity_unavailable",
            "身份服务暂时不可用，请稍后重试",
        )
    }
    pub fn busy() -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code: "identity_busy",
            message: "身份服务繁忙，请稍后重试".into(),
            retry_after: Some(1),
        }
    }
    pub fn credentials() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "账号或密码不正确",
        )
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "user_not_found", "未找到该用户")
    }
}
impl From<AuthError> for IdentityError {
    fn from(error: AuthError) -> Self {
        match error {
            AuthError::EmailDelivery => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "email_delivery_failed",
                "验证码邮件未能发送，请稍后重试",
            ),
            AuthError::Validation(message) => {
                Self::new(StatusCode::BAD_REQUEST, "invalid_input", message)
            }
            AuthError::Conflict(message) => {
                Self::new(StatusCode::CONFLICT, "state_conflict", message)
            }
            AuthError::RateLimited {
                message,
                retry_after,
            } => Self {
                status: StatusCode::TOO_MANY_REQUESTS,
                code: "rate_limited",
                message,
                retry_after: Some(retry_after),
            },
            _ => Self::unavailable(),
        }
    }
}
impl From<crate::errors::ApiError> for IdentityError {
    fn from(error: crate::errors::ApiError) -> Self {
        Self::new(error.0, error.1, error.2)
    }
}
impl IntoResponse for IdentityError {
    fn into_response(self) -> Response {
        let mut response=(self.status,Json(json!({"error":{"message":self.message,"code":self.code},"message":self.message,"code":self.code,"resumable":false}))).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-store"),
        );
        if let Some(seconds) = self.retry_after {
            response.headers_mut().insert(
                header::RETRY_AFTER,
                header::HeaderValue::from_str(&seconds.to_string()).expect("integer header"),
            );
        }
        response
    }
}
