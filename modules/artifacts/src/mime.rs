// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{ArtifactError, Result};
pub(crate) fn content_type(filename: &str) -> Result<&'static str> {
    if filename.is_empty()
        || filename.len() > 240
        || filename.starts_with('.')
        || filename
            .chars()
            .any(|c| c.is_control() || matches!(c, '/' | '\\' | '"' | ':' | '%' | '?' | '#'))
    {
        return Err(ArtifactError::Invalid);
    }
    Ok(
        match std::path::Path::new(filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "flac" => "audio/flac",
            "m4a" => "audio/mp4",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            "pdf" => "application/pdf",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "html" | "htm" => "text/html; charset=utf-8",
            "css" => "text/css; charset=utf-8",
            "js" | "mjs" => "text/javascript; charset=utf-8",
            "json" => "application/json",
            "svg" => "image/svg+xml",
            "woff" => "font/woff",
            "woff2" => "font/woff2",
            "zip" => "application/zip",
            "txt" | "md" => "text/plain; charset=utf-8",
            _ => "application/octet-stream",
        },
    )
}
