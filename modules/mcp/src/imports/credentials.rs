use super::PendingSecret;
use crate::{McpError, Result};
use exoharness_contracts::capability::{
    CapabilityManifest, CapabilitySpec, EnvironmentBinding, McpTransport, SecretReference,
    is_sensitive_http_header,
};
pub(super) fn extract(
    manifest: &mut CapabilityManifest,
    index: usize,
    secrets: &mut Vec<PendingSecret>,
) -> Result<()> {
    let CapabilitySpec::McpServer(spec) = &mut manifest.spec else {
        return Err(McpError::Configuration);
    };
    let (bindings, all_sensitive) = match &mut spec.transport {
        McpTransport::StreamableHttp { headers, .. } => (headers, false),
        McpTransport::Stdio { environment, .. } => (environment, true),
    };
    for (header, binding) in bindings {
        let EnvironmentBinding::Literal { value } = binding else {
            continue;
        };
        if value.len() > 8192 || value.is_empty() || value.chars().any(char::is_control) {
            return Err(McpError::Import("Header或环境变量值为空、过长或包含换行"));
        }
        if !all_sensitive && !is_sensitive_http_header(header) {
            continue;
        }
        let name = format!("imported-{}", manifest.secret_refs.len() + 1);
        let credential = std::mem::take(value);
        *binding = EnvironmentBinding::SecretRef {
            reference: name.clone(),
        };
        manifest.secret_refs.push(SecretReference {
            name: name.clone(),
            provider: "local".into(),
            key: format!("pending-{}", secrets.len()),
        });
        secrets.push(PendingSecret {
            index,
            name,
            value: credential,
        });
    }
    Ok(())
}
