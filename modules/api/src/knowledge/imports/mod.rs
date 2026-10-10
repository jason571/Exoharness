//! Trusted document parsing through the existing isolated runtime, never host Python.
use super::{KnowledgeState, Params, id, new_id, scope};
use crate::{
    auth::Principal,
    errors::{ApiError, storage},
};
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use exoharness_sandbox::{Limits, RunSpec, Runtime};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, sync::Arc};
use tokio::sync::Semaphore;

static PARSERS: Semaphore = Semaphore::const_new(2);
const MAX_SOURCE: usize = 4 * 1024 * 1024;

fn bad(code: &'static str, message: &'static str) -> ApiError {
    ApiError(StatusCode::UNPROCESSABLE_ENTITY, code, message)
}
fn parsing_error(code: &str) -> ApiError {
    match code {
        "pdf_without_text_layer" => bad(
            "pdf_without_text_layer",
            "PDF 没有可读取的文字层；暂不支持扫描件或图片 OCR",
        ),
        "encrypted_pdf" => bad("encrypted_pdf", "请先解除 PDF 加密后再导入"),
        "parser_dependency_missing" => ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "document_parser_unavailable",
            "文档解析依赖尚未就绪；本次未入库",
        ),
        "extracted_text_too_large"
        | "structured_document_too_large"
        | "too_many_pages"
        | "package_too_large"
        | "table_too_large" => bad(
            "document_too_large",
            "文档解析后超出安全容量；未拆分表格或保存部分内容，请减少文档范围",
        ),
        "no_text" => bad(
            "no_document_text",
            "文档中未找到可读取的文字；图片内容暂不识别",
        ),
        "unsafe_xml" | "unsafe_package" | "macros_not_supported" => bad(
            "unsafe_document",
            "文档含不支持或不安全的包内容；本次未入库",
        ),
        _ => bad(
            "invalid_document",
            "文档损坏或结构不支持，请检查文件后重新导入",
        ),
    }
}

pub(super) async fn import(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
    Json(mut body): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let owner = scope(&state, &principal, &params)?;
    // Check library existence before consuming parsing resources.
    let check_owner = owner.clone();
    storage(state.store.clone(), move |s| {
        s.knowledge_documents(&check_owner, None, None)
    })
    .await?;
    let filename = body["filename"]
        .as_str()
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 512
                && !s.chars().any(|c| c.is_control() || matches!(c, '/' | '\\'))
        })
        .ok_or_else(|| bad("invalid_filename", "文件名无效"))?
        .to_owned();
    let extension = filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "docx" | "pdf") {
        return Err(bad(
            "unsupported_document",
            "此接口支持 DOCX 和文字型 PDF；旧版 DOC 请先转换为 DOCX",
        ));
    }
    let encoded = body["file_base64"]
        .as_str()
        .filter(|s| s.len() <= MAX_SOURCE.div_ceil(3) * 4)
        .ok_or_else(|| bad("document_too_large", "文件超过 4 MB 或上传数据无效"))?;
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| bad("invalid_upload", "上传文件编码无效"))?;
    if bytes.is_empty() || bytes.len() > MAX_SOURCE {
        return Err(bad("document_too_large", "文件为空或超过 4 MB"));
    }
    let sandbox = state
        .sandbox
        .as_ref()
        .ok_or(ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "document_parser_unavailable",
            "隔离文档解析服务尚未就绪；本次未入库",
        ))?
        .clone();
    let _permit = PARSERS.try_acquire().map_err(|_| {
        ApiError(
            StatusCode::TOO_MANY_REQUESTS,
            "document_parser_busy",
            "文档解析繁忙，请稍后再试；本次未入库",
        )
    })?;
    let document_id = new_id("document")?;
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let size = bytes.len();
    let input = format!("input.{extension}");
    let files = BTreeMap::from([
        (input.clone(), bytes),
        (
            "parse_document.py".into(),
            include_bytes!("parse_document.py").to_vec(),
        ),
    ]);
    let workspace = sandbox.create_workspace(&document_id, files).map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "document_store_unavailable",
            "私有文件保存失败；本次未入库",
        )
    })?;
    // The original is private, never exposed on the media service or in API responses.
    let source = json!({"filename":filename,"path":workspace.path().join(&input),"sha256":hash,"size_bytes":size});
    let job = sandbox
        .start(RunSpec {
            invocation_id: document_id.clone(),
            workspace,
            read_only_package: None,
            runtime: Runtime::Python,
            entrypoint: "parse_document.py".into(),
            arguments: vec![extension],
            limits: Limits {
                output_bytes: 4 * 1024 * 1024,
                ..Limits::default()
            },
        })
        .map_err(|_| {
            ApiError(
                StatusCode::SERVICE_UNAVAILABLE,
                "document_parser_failed",
                "隔离解析未能启动；本次未入库",
            )
        })?;
    // Dropping this future drops JobHandle, which cancels the owned container.
    let receipt = job.finish().await.map_err(|_| {
        ApiError(
            StatusCode::SERVICE_UNAVAILABLE,
            "document_parser_failed",
            "隔离解析未完成；本次未入库",
        )
    })?;
    if receipt.status != "completed"
        || receipt.exit_code != Some(0)
        || receipt.output_truncated
        || !receipt.cleanup_confirmed
    {
        return Err(bad(
            "document_parse_incomplete",
            "文档解析未完整完成；本次未入库，请检查文件后重试",
        ));
    }
    let mut parsed: Value = serde_json::from_str(&receipt.stdout)
        .map_err(|_| bad("invalid_parser_result", "文档解析结果无效；本次未入库"))?;
    if let Some(error) = parsed["error"].as_str() {
        return Err(parsing_error(error));
    }
    let content = parsed["content"]
        .as_str()
        .ok_or_else(|| bad("invalid_parser_result", "文档解析结果缺少文字；本次未入库"))?
        .to_owned();
    parsed
        .as_object_mut()
        .ok_or_else(|| bad("invalid_parser_result", "文档解析结果无效"))?
        .remove("content");
    body.as_object_mut()
        .ok_or_else(|| bad("invalid_upload", "上传数据无效"))?
        .remove("file_base64");
    body["title"] = json!(filename);
    body["content"] = json!(content);
    body["source_uri"] = Value::Null;
    if body.get("tags").is_none() {
        body["tags"] = json!([]);
    }
    let saved_id = document_id.clone();
    let saved_owner = owner.clone();
    storage(state.store.clone(), move |s| {
        s.save_imported_document(&saved_owner, &saved_id, &body, &parsed, &source)
    })
    .await?;
    let manifest = storage(state.store, move |s| {
        s.document_blocks(&owner, &document_id)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(manifest)))
}

pub(super) async fn blocks(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let document = id(&params)?.to_owned();
    Ok(Json(
        storage(state.store, move |s| s.document_blocks(&owner, &document)).await?,
    ))
}
#[derive(Deserialize)]
pub(super) struct BlockRange {
    revision: i64,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize {
    100
}
pub(super) async fn block(
    State(state): State<KnowledgeState>,
    Extension(principal): Extension<Principal>,
    Path(params): Path<Params>,
    Query(range): Query<BlockRange>,
) -> Result<Json<Value>, ApiError> {
    let owner = scope(&state, &principal, &params)?;
    let document = id(&params)?.to_owned();
    let block = params
        .get("block")
        .ok_or_else(|| bad("missing_block", "缺少内容块标识"))?
        .clone();
    Ok(Json(
        storage(state.store, move |s| {
            s.read_document_block(
                &owner,
                &document,
                &block,
                range.revision,
                range.offset,
                range.limit,
            )
        })
        .await?,
    ))
}

pub(super) type ParserSandbox = Option<Arc<exoharness_sandbox::Sandbox>>;
