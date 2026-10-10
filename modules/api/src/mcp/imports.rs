use crate::{auth::Principal, errors::ApiError};
use axum::{Extension, Json, body::Bytes, extract::State};
use exoharness_contracts::capability::CapabilityScope;
use exoharness_mcp::{ImportRequest, Mcp, McpError};
use exoharness_storage_postgres::RegistryAccess;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HttpImport {
    name: String,
    version: String,
    description: String,
    namespace: String,
    url: String,
    #[serde(default)]
    bearer_token: Option<String>,
    scope: CapabilityScope,
    #[serde(default)]
    actor: Option<String>,
}
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, McpError> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| McpError::Import("请求必须使用UTF-8 JSON"))?;
    let value = exoharness_contracts::strict_json::parse(text)
        .map_err(|_| McpError::Import("请求必须是字段不重复的完整JSON"))?;
    serde_json::from_value(value).map_err(|_| McpError::Import("MCP导入字段或类型不正确"))
}
pub(super) async fn http(
    State(mcp): State<Arc<Mcp>>,
    Extension(principal): Extension<Principal>,
    bytes: Bytes,
) -> Result<Json<Value>, ApiError> {
    let records=super::work(mcp,move|mcp|{
        let request:HttpImport=decode(&bytes)?;
        let _provenance=request.actor;
        let headers=match request.bearer_token {Some(token) if !token.is_empty()=>json!({"Authorization":format!("Bearer {token}")}),_=>json!({})};
        let mut servers=serde_json::Map::new();
        servers.insert(request.name,json!({"type":"streamableHttp","url":request.url,"namespace":request.namespace,"version":request.version,"description":request.description,"headers":headers}));
        let request=ImportRequest{content:json!({"mcpServers":servers}).to_string(),file_name:None,scope:request.scope,actor:None};
        mcp.import(&RegistryAccess::Administrator,&request,&principal.id)
    }).await?;
    Ok(Json(
        json!({"capabilities":records,"warnings":["配置已登记；导入未连接或启动服务。"]}),
    ))
}
pub(super) async fn import(
    State(mcp): State<Arc<Mcp>>,
    Extension(principal): Extension<Principal>,
    bytes: Bytes,
) -> Result<Json<Value>, ApiError> {
    let records = super::work(mcp, move |mcp| {
        let request: ImportRequest = decode(&bytes)?;
        mcp.import(&RegistryAccess::Administrator, &request, &principal.id)
    })
    .await?;
    Ok(Json(
        json!({"capabilities":records,"warnings":["配置已登记；敏感 Header 已独立保存且不回显。导入未启动服务或调用工具。"]}),
    ))
}
