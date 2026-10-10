// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use exoharness_skills::SkillError;
use serde_json::json;

pub(super) struct SkillHttpError(pub SkillError);
impl IntoResponse for SkillHttpError {
    fn into_response(self) -> Response {
        match self.0 {
            SkillError::Store(error) => crate::errors::ApiError::from(error).into_response(),
            SkillError::Io => crate::errors::ApiError(StatusCode::SERVICE_UNAVAILABLE,
                "skill_store_unavailable", "Skill 内容存储暂时不可用，未执行上传代码").into_response(),
            SkillError::Validation(message) => (StatusCode::BAD_REQUEST,
                Json(json!({"error":{"code":"invalid_skill_package","message":message},"resumable":false}))).into_response(),
        }
    }
}
