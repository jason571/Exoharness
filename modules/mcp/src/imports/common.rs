// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::ImportRequest;
use crate::{McpError, Result};
use exoharness_contracts::capability::{
    CapabilityManifest, CapabilitySource, CapabilitySpec, EnvironmentBinding, McpServerSpec,
    McpTransport, ResourceLimits,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(rename = "type", default)]
    transport: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default, alias = "protocolVersion")]
    protocol_version: Option<String>,
    #[serde(default, alias = "maxConcurrency")]
    max_concurrency: Option<u32>,
}
pub(super) fn manifest(
    request: &ImportRequest,
    name: &str,
    value: &Value,
) -> Result<CapabilityManifest> {
    let config: Config = serde_json::from_value(value.clone())
        .map_err(|_| McpError::Import("服务字段或类型错误，请检查type/url/command/headers"))?;
    let (source, transport, permissions) = match (&config.url, &config.command) {
        (Some(url), None) => {
            if !config.args.is_empty() || !config.env.is_empty() {
                return Err(McpError::Import("HTTP服务不能携带本地命令参数或环境"));
            }
            if config.transport.as_deref().is_some_and(|kind| {
                !matches!(
                    kind,
                    "streamableHttp" | "streamable_http" | "streamable-http" | "http"
                )
            }) {
                return Err(McpError::Import("当前地址需使用Streamable HTTP传输"));
            }
            (
                CapabilitySource::Remote { url: url.clone() },
                McpTransport::StreamableHttp {
                    url: url.clone(),
                    auth_secret: None,
                    headers: config
                        .headers
                        .into_iter()
                        .map(|(name, value)| (name, EnvironmentBinding::Literal { value }))
                        .collect(),
                },
                vec!["network.egress".into()],
            )
        }
        (None, Some(command)) => {
            if !config.headers.is_empty()
                || config
                    .transport
                    .as_deref()
                    .is_some_and(|kind| kind != "stdio")
            {
                return Err(McpError::Import(
                    "本地MCP需要stdio配置，不能附加HTTP Header",
                ));
            }
            let mut command = vec![command.clone()];
            command.extend(config.args);
            (
                CapabilitySource::Uploaded {
                    file_name: request
                        .file_name
                        .clone()
                        .unwrap_or_else(|| "pasted-mcp.json".into()),
                },
                McpTransport::Stdio {
                    command,
                    environment: config
                        .env
                        .into_iter()
                        .map(|(name, value)| (name, EnvironmentBinding::Literal { value }))
                        .collect(),
                    working_directory: None,
                },
                vec!["process.spawn".into()],
            )
        }
        _ => return Err(McpError::Import("必须只选择url或command其中一种连接方式")),
    };
    let limits = ResourceLimits {
        max_concurrency: config.max_concurrency.unwrap_or(1),
        ..Default::default()
    };
    Ok(CapabilityManifest {
        schema_version: 1,
        name: name.into(),
        version: config.version.unwrap_or_else(|| "0.1.0".into()),
        description: config
            .description
            .unwrap_or_else(|| format!("MCP服务{name}")),
        scope: request.scope.clone(),
        source,
        permissions,
        secret_refs: vec![],
        spec: CapabilitySpec::McpServer(McpServerSpec {
            namespace: config.namespace.unwrap_or_else(|| name.into()),
            transport,
            protocol_version: config.protocol_version,
            limits,
        }),
    })
}
