// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_skills::{ImportRequest, SkillError};

pub(super) fn upload(bytes: &[u8]) -> Result<ImportRequest, SkillError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| SkillError::Validation("上传请求必须使用 UTF-8".into()))?;
    let value = exoharness_contracts::strict_json::parse(text)
        .map_err(|_| SkillError::Validation("上传请求必须是字段不重复的完整 JSON".into()))?;
    serde_json::from_value(value).map_err(|_| {
        SkillError::Validation("上传参数类型不正确；files 需要是文件数组，不能是编码字符串".into())
    })
}
