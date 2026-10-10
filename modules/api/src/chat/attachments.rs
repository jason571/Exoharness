//! Chat input files are persisted once; model and history receive references only.
use crate::errors::ApiError;
use axum::http::StatusCode;
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_artifacts::Artifacts;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs::File, io::Write};

// 64 MiB decoded aggregate plus Base64 expansion and bounded JSON metadata.
pub(super) const MAX_REQUEST_BYTES: usize = 96 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 64 * 1024 * 1024;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    #[serde(rename = "type")]
    kind: String,
    name: String,
    #[serde(default)]
    mime_type: String,
    #[serde(default)]
    data: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    size: Option<u64>,
}
pub(super) struct Prepared {
    name: String,
    kind: String,
    file: Option<File>,
    text: Option<String>,
}
fn invalid(message: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, "invalid_attachment", message)
}
pub(super) fn prepare(inputs: Vec<Value>) -> Result<Vec<Prepared>, ApiError> {
    if inputs.len() > 8 {
        return Err(invalid("一次最多添加8个附件"));
    }
    let mut total = 0usize;
    let mut names = std::collections::BTreeSet::new();
    let mut prepared = Vec::new();
    for value in inputs {
        let input: Input =
            serde_json::from_value(value).map_err(|_| invalid("附件格式不正确，请重新添加文件"))?;
        if input.name.is_empty()
            || input.name.len() > 240
            || input.name.starts_with('.')
            || input
                .name
                .chars()
                .any(|c| c.is_control() || matches!(c, '/' | '\\' | '"' | ':' | '%' | '?' | '#'))
            || !names.insert(input.name.clone())
        {
            return Err(invalid("附件文件名不正确或重复，请重新添加文件"));
        }
        let limit: usize = match input.kind.as_str() {
            "audio" | "video" | "file" => 32 * 1024 * 1024,
            "image" => 5 * 1024 * 1024,
            "text" => 512 * 1024,
            _ => return Err(invalid("附件类型不支持")),
        };
        let _ = (input.mime_type, input.size);
        let (bytes, text) = if input.kind == "text" {
            if !input.data.is_empty() {
                return Err(invalid("文字附件格式不正确"));
            }
            (input.content.as_bytes().to_vec(), Some(input.content))
        } else {
            if input.data.is_empty() || !input.content.is_empty() {
                return Err(invalid("附件内容未上传，请重新添加文件"));
            }
            if input.data.len() > limit.div_ceil(3) * 4 {
                return Err(too_large());
            }
            (
                STANDARD
                    .decode(input.data)
                    .map_err(|_| invalid("附件编码不正确，请重新添加文件"))?,
                None,
            )
        };
        total += bytes.len();
        if bytes.len() > limit || total > MAX_TOTAL_BYTES {
            return Err(too_large());
        }
        let file = if text.is_none() {
            if bytes.is_empty() {
                return Err(invalid("附件不能为空"));
            }
            let mut file = tempfile::tempfile().map_err(|_| unavailable())?;
            file.write_all(&bytes).map_err(|_| unavailable())?;
            Some(file)
        } else {
            None
        };
        prepared.push(Prepared {
            name: input.name,
            kind: input.kind,
            file,
            text,
        });
    }
    Ok(prepared)
}
pub(super) fn too_large() -> ApiError {
    ApiError(
        StatusCode::PAYLOAD_TOO_LARGE,
        "attachments_too_large",
        "音频或文件单个最多32 MiB，图片最多5 MiB；附件总大小最多64 MiB",
    )
}
pub(super) fn unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "attachments_unavailable",
        "附件保存失败，本次没有调用模型或执行工具，请重新发送",
    )
}
pub(super) fn save(
    inputs: Vec<Prepared>,
    artifacts: &Artifacts,
    owner: &str,
    session: &str,
    run: &str,
) -> Result<Vec<Value>, ApiError> {
    inputs.into_iter().map(|input| {
        if let Some(text) = input.text {
            Ok(json!({"type":"text","name":input.name,"content":text,"size":text.len()}))
        } else {
            let saved = artifacts.publish_file(owner, session, run, &input.name, input.file.ok_or_else(unavailable)?).map_err(|_| unavailable())?;
            Ok(json!({"type":input.kind,"name":input.name,"mime_type":saved.artifact.content_type,"size":saved.artifact.size_bytes,"attachment_id":saved.artifact.id,"artifact_id":saved.artifact.id,"sha256":saved.artifact.sha256,"url":saved.url,"download_url":saved.download_url}))
        }
    }).collect()
}
