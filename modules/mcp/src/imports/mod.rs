//! Configuration parsing is pure. Network and code are not import operations.
mod common;
mod credentials;
use crate::{McpError, Result};
use exoharness_contracts::capability::{
    CapabilityManifest, CapabilityScope, CapabilitySpec, ValidatedCapability,
};
use serde::Deserialize;
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub content: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default = "global")]
    pub scope: CapabilityScope,
    #[serde(default)]
    pub actor: Option<String>,
}
fn global() -> CapabilityScope {
    CapabilityScope::Global
}
pub struct ParsedImport {
    pub(crate) manifests: Vec<CapabilityManifest>,
    pub(crate) secrets: Vec<PendingSecret>,
    pub(crate) administrator_manifest: bool,
}
pub(crate) struct PendingSecret {
    pub index: usize,
    pub name: String,
    pub value: String,
}
pub fn parse_import(request: &ImportRequest) -> Result<ParsedImport> {
    if request.content.len() > 4 * 1024 * 1024 {
        return Err(McpError::Import("JSON配置超过4MiB"));
    }
    let value = exoharness_contracts::strict_json::parse(&request.content)
        .map_err(|_| McpError::Import("需要完整且无重复字段的JSON配置"))?;
    let mut parsed = ParsedImport {
        manifests: Vec::new(),
        secrets: Vec::new(),
        administrator_manifest: value.get("spec").is_some(),
    };
    if value.get("spec").is_some() {
        let mut manifest: CapabilityManifest = serde_json::from_value(value)
            .map_err(|_| McpError::Import("Manifest字段或类型不正确"))?;
        if !matches!(manifest.spec, CapabilitySpec::McpServer(_)) {
            return Err(McpError::Import("请选择MCP配置"));
        }
        manifest.scope = request.scope.clone();
        credentials::extract(&mut manifest, 0, &mut parsed.secrets)?;
        parsed.manifests.push(manifest);
    } else {
        let object = value
            .as_object()
            .ok_or(McpError::Import("顶层需要是JSON对象"))?;
        if object.len() != 1 {
            return Err(McpError::Import("MCP配置顶层仅接受mcpServers或mcp_servers"));
        }
        let servers = object
            .get("mcpServers")
            .or_else(|| object.get("mcp_servers"))
            .and_then(serde_json::Value::as_object)
            .ok_or(McpError::Import("需要mcpServers服务对象"))?;
        if servers.is_empty() || servers.len() > 64 {
            return Err(McpError::Import("一次需要1至64个服务"));
        }
        for (index, (name, config)) in servers.iter().enumerate() {
            let manifest = common::manifest(request, name, config)?;
            parsed.manifests.push(manifest);
            credentials::extract(&mut parsed.manifests[index], index, &mut parsed.secrets)?;
        }
    }
    let mut identities = BTreeMap::new();
    for manifest in &parsed.manifests {
        if identities.insert(manifest.id(), ()).is_some() {
            return Err(McpError::Import("服务名称重复"));
        }
        ValidatedCapability::new(manifest.clone(), None)
            .map_err(|_| McpError::Import("服务地址、名称、权限或Header不符合配置规则"))?;
    }
    Ok(parsed)
}
