// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use std::collections::HashSet;
use std::net::IpAddr;
use std::path::{Component, Path};

use http::{HeaderName, HeaderValue};
use sha2::{Digest, Sha256};

use super::ManifestError;
use super::{
    CapabilityManifest, CapabilitySource, CapabilitySpec, EnvironmentBinding,
    MANIFEST_SCHEMA_VERSION, McpTransport, RunnerSpec,
};

const MAX_NAME_LEN: usize = 128;
const MAX_DESCRIPTION_LEN: usize = 16 * 1024;
const MAX_PERMISSION_LEN: usize = 128;

pub fn validate_and_normalize_manifest(
    mut manifest: CapabilityManifest,
) -> Result<CapabilityManifest, ManifestError> {
    if manifest.schema_version != MANIFEST_SCHEMA_VERSION {
        return Err(ManifestError::Validation(format!(
            "unsupported manifest schema_version {}, expected {}",
            manifest.schema_version, MANIFEST_SCHEMA_VERSION
        )));
    }
    validate_identifier("name", &manifest.name, MAX_NAME_LEN)?;
    validate_version(&manifest.version)?;
    if manifest.description.trim().is_empty() {
        return Err(ManifestError::Validation(
            "description must not be empty".into(),
        ));
    }
    if manifest.description.len() > MAX_DESCRIPTION_LEN {
        return Err(ManifestError::Validation(format!(
            "description exceeds {MAX_DESCRIPTION_LEN} bytes"
        )));
    }

    match &manifest.scope {
        super::CapabilityScope::Global => {}
        super::CapabilityScope::User { user_id } => {
            validate_identifier("scope.user_id", user_id, 128)?;
        }
        super::CapabilityScope::Project { project } => {
            validate_identifier("scope.project", project, MAX_NAME_LEN)?;
        }
        super::CapabilityScope::Session { session_id } => {
            if session_id.trim().is_empty() || session_id.len() > 256 {
                return Err(ManifestError::Validation(
                    "scope.session_id must contain 1..=256 bytes".into(),
                ));
            }
        }
    }

    validate_source(&manifest.source, &manifest.spec)?;

    let mut permission_set = HashSet::new();
    for permission in &manifest.permissions {
        validate_identifier("permission", permission, MAX_PERMISSION_LEN)?;
        if !permission_set.insert(permission.clone()) {
            return Err(ManifestError::Validation(format!(
                "duplicate permission {permission:?}"
            )));
        }
    }
    manifest.permissions.sort();

    let mut secret_names = HashSet::new();
    for secret in &manifest.secret_refs {
        validate_identifier("secret_ref.name", &secret.name, MAX_NAME_LEN)?;
        validate_identifier("secret_ref.provider", &secret.provider, MAX_NAME_LEN)?;
        if secret.key.trim().is_empty() || secret.key.len() > 512 {
            return Err(ManifestError::Validation(
                "secret_ref.key must contain 1..=512 bytes".into(),
            ));
        }
        if !secret_names.insert(secret.name.clone()) {
            return Err(ManifestError::Validation(format!(
                "duplicate secret reference {:?}",
                secret.name
            )));
        }
    }
    manifest.secret_refs.sort();

    validate_spec(&manifest, &secret_names)?;
    Ok(manifest)
}

pub fn manifest_digest(manifest: &CapabilityManifest) -> Result<String, ManifestError> {
    let bytes = serde_json::to_vec(manifest)?;
    let digest = Sha256::digest(bytes);
    Ok(hex_encode(&digest))
}

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub fn normalize_source_digest(value: Option<String>) -> Result<Option<String>, ManifestError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.to_ascii_lowercase();
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ManifestError::Validation(
            "source_digest must be a 64-character SHA-256 hex string".into(),
        ));
    }
    Ok(Some(value))
}

fn validate_identifier(field: &str, value: &str, max_len: usize) -> Result<(), ManifestError> {
    let bytes = value.as_bytes();
    let valid = !bytes.is_empty()
        && bytes.len() <= max_len
        && is_ascii_lowercase_or_digit(bytes[0])
        && bytes
            .iter()
            .all(|b| is_ascii_lowercase_or_digit(*b) || matches!(b, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err(ManifestError::Validation(format!(
            "{field} must match [a-z0-9][a-z0-9._-]{{0,{}}}",
            max_len - 1
        )))
    }
}

fn is_ascii_lowercase_or_digit(byte: u8) -> bool {
    byte.is_ascii_lowercase() || byte.is_ascii_digit()
}

fn validate_version(value: &str) -> Result<(), ManifestError> {
    if value.len() > 64 || semver::Version::parse(value).is_err() {
        return Err(ManifestError::Validation(
            "version must be a valid semantic version of at most 64 bytes".into(),
        ));
    }
    Ok(())
}

fn validate_source(source: &CapabilitySource, spec: &CapabilitySpec) -> Result<(), ManifestError> {
    match source {
        CapabilitySource::Local { path } => validate_nonempty("source.path", path, 4096),
        CapabilitySource::Uploaded { file_name } => {
            validate_nonempty("source.file_name", file_name, 512)?;
            let path = Path::new(file_name);
            if path.file_name().and_then(|name| name.to_str()) != Some(file_name) {
                return Err(ManifestError::Validation(
                    "source.file_name must be a base file name".into(),
                ));
            }
            Ok(())
        }
        CapabilitySource::Git { url, revision } => {
            validate_nonempty("source.url", url, 4096)?;
            validate_nonempty("source.revision", revision, 512)
        }
        CapabilitySource::Remote { url } => {
            if matches!(
                spec,
                CapabilitySpec::McpServer(super::McpServerSpec {
                    transport: McpTransport::StreamableHttp { .. },
                    ..
                })
            ) {
                validate_mcp_http_url("source.url", url)
            } else {
                validate_http_url("source.url", url)
            }
        }
        CapabilitySource::Registry {
            registry,
            package,
            version,
        } => {
            validate_http_url("source.registry", registry)?;
            validate_nonempty("source.package", package, 512)?;
            validate_version(version)
        }
    }
}

fn validate_spec(
    manifest: &CapabilityManifest,
    secret_names: &HashSet<String>,
) -> Result<(), ManifestError> {
    match &manifest.spec {
        CapabilitySpec::Skill(spec) => {
            validate_relative_path("skill.content_root", &spec.content_root)?;
            validate_relative_path("skill.entrypoint", &spec.entrypoint)?;
            if spec.executable_scripts {
                return Err(ManifestError::Validation(
                    "skills cannot enable executable_scripts; register scripts as tool nodes"
                        .into(),
                ));
            }
        }
        CapabilitySpec::McpServer(spec) => {
            validate_identifier("mcp.namespace", &spec.namespace, MAX_NAME_LEN)?;
            if spec.artifact_origins.len() > 8 {
                return Err(ManifestError::Validation(
                    "MCP artifact_origins accepts at most 8 origins".into(),
                ));
            }
            let mut origins = HashSet::new();
            for origin in &spec.artifact_origins {
                validate_mcp_http_url("mcp.artifact_origins", origin)?;
                let parsed: http::Uri = origin
                    .parse()
                    .map_err(|_| ManifestError::Validation("Invalid MCP artifact origin".into()))?;
                let port = parsed
                    .port_u16()
                    .unwrap_or(if parsed.scheme_str() == Some("https") {
                        443
                    } else {
                        80
                    });
                if !matches!(parsed.path(), "" | "/")
                    || parsed.query().is_some()
                    || origin.contains('#')
                    || !origins.insert((
                        parsed.scheme_str().unwrap().to_owned(),
                        parsed.host().unwrap().to_lowercase(),
                        port,
                    ))
                {
                    return Err(ManifestError::Validation("MCP artifact_origins must contain unique HTTP(S) origins without paths, queries or fragments".into()));
                }
            }
            if spec.limits.max_concurrency == 0 || spec.limits.max_concurrency > 1024 {
                return Err(ManifestError::Validation(
                    "MCP max_concurrency must be in 1..=1024".into(),
                ));
            }
            match &spec.transport {
                McpTransport::Stdio {
                    command,
                    environment,
                    working_directory,
                } => {
                    validate_command("mcp.stdio.command", command)?;
                    validate_environment(environment, secret_names)?;
                    if let Some(path) = working_directory {
                        validate_nonempty("mcp.stdio.working_directory", path, 4096)?;
                    }
                }
                McpTransport::StreamableHttp {
                    url,
                    auth_secret,
                    headers,
                } => {
                    validate_mcp_http_url("mcp.streamable_http.url", url)?;
                    if let Some(reference) = auth_secret {
                        require_secret_ref(reference, secret_names)?;
                    }
                    validate_http_headers(headers, secret_names)?;
                    if auth_secret.is_some()
                        && headers
                            .keys()
                            .any(|name| name.eq_ignore_ascii_case("authorization"))
                    {
                        return Err(ManifestError::Validation(
                            "streamable HTTP MCP cannot set both auth_secret and an Authorization header"
                                .into(),
                        ));
                    }
                }
            }
        }
        CapabilitySpec::ToolNode(spec) => {
            if spec.tools.is_empty() {
                return Err(ManifestError::Validation(
                    "tool node must declare at least one tool".into(),
                ));
            }
            let mut tool_names = HashSet::new();
            for tool in &spec.tools {
                validate_identifier("tool.name", &tool.name, MAX_NAME_LEN)?;
                if !tool_names.insert(tool.name.clone()) {
                    return Err(ManifestError::Validation(format!(
                        "duplicate tool name {:?}",
                        tool.name
                    )));
                }
                if tool.description.trim().is_empty() {
                    return Err(ManifestError::Validation(format!(
                        "tool {:?} description must not be empty",
                        tool.name
                    )));
                }
                validate_json_schema_root("tool.input_schema", &tool.input_schema)?;
                if let Some(schema) = &tool.output_schema {
                    validate_json_schema_root("tool.output_schema", schema)?;
                }
            }
            if spec.limits.timeout_ms == 0 || spec.limits.timeout_ms > 24 * 60 * 60 * 1000 {
                return Err(ManifestError::Validation(
                    "tool timeout_ms must be in 1..=86400000".into(),
                ));
            }
            if spec.limits.max_output_bytes == 0 || spec.limits.max_output_bytes > 64 * 1024 * 1024
            {
                return Err(ManifestError::Validation(
                    "tool max_output_bytes must be in 1..=67108864".into(),
                ));
            }
            if spec.limits.max_concurrency == 0 || spec.limits.max_concurrency > 1024 {
                return Err(ManifestError::Validation(
                    "tool max_concurrency must be in 1..=1024".into(),
                ));
            }
            if spec.limits.memory_bytes == Some(0) {
                return Err(ManifestError::Validation(
                    "tool memory_bytes must be greater than zero when set".into(),
                ));
            }
            match &spec.runner {
                RunnerSpec::Process {
                    command,
                    environment,
                    working_directory,
                } => {
                    validate_command("tool.process.command", command)?;
                    validate_environment(environment, secret_names)?;
                    if let Some(path) = working_directory {
                        validate_relative_path("tool.process.working_directory", path)?;
                    }
                }
                RunnerSpec::Oci { image, .. } => validate_nonempty("tool.oci.image", image, 512)?,
                RunnerSpec::Wasm { module, .. } => {
                    validate_relative_path("tool.wasm.module", module)?
                }
            }
            if spec.limits.timeout_ms == 0
                || spec.limits.max_output_bytes == 0
                || spec.limits.max_concurrency == 0
            {
                return Err(ManifestError::Validation(
                    "tool resource limits must be greater than zero".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_environment(
    environment: &std::collections::BTreeMap<String, EnvironmentBinding>,
    secret_names: &HashSet<String>,
) -> Result<(), ManifestError> {
    for (name, binding) in environment {
        if name.is_empty()
            || name.len() > 128
            || !name
                .bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
        {
            return Err(ManifestError::Validation(format!(
                "environment variable {name:?} must use [A-Z0-9_]"
            )));
        }
        if let EnvironmentBinding::SecretRef { reference } = binding {
            require_secret_ref(reference, secret_names)?;
        }
    }
    Ok(())
}

fn validate_http_headers(
    headers: &std::collections::BTreeMap<String, EnvironmentBinding>,
    secret_names: &HashSet<String>,
) -> Result<(), ManifestError> {
    if headers.len() > 64 {
        return Err(ManifestError::Validation(
            "streamable HTTP MCP may declare at most 64 headers".into(),
        ));
    }
    let mut normalized_names = HashSet::new();
    for (name, binding) in headers {
        if name.len() > 128 || HeaderName::from_bytes(name.as_bytes()).is_err() {
            return Err(ManifestError::Validation(format!(
                "HTTP header name {name:?} is invalid"
            )));
        }
        if is_reserved_transport_header(name) {
            return Err(ManifestError::Validation(format!(
                "HTTP header {name:?} is managed by the MCP transport"
            )));
        }
        if !normalized_names.insert(name.to_ascii_lowercase()) {
            return Err(ManifestError::Validation(format!(
                "HTTP header {name:?} is duplicated with different casing"
            )));
        }
        match binding {
            EnvironmentBinding::Literal { value } => {
                if is_sensitive_http_header(name) {
                    return Err(ManifestError::Validation(format!(
                        "HTTP header {name:?} may contain credentials and must use a SecretRef"
                    )));
                }
                if value.len() > 8 * 1024 || HeaderValue::try_from(value.as_str()).is_err() {
                    return Err(ManifestError::Validation(format!(
                        "HTTP header {name:?} has an invalid value"
                    )));
                }
            }
            EnvironmentBinding::SecretRef { reference } => {
                require_secret_ref(reference, secret_names)?;
            }
        }
    }
    Ok(())
}

fn is_reserved_transport_header(name: &str) -> bool {
    [
        "accept",
        "content-length",
        "content-type",
        "connection",
        "host",
        "last-event-id",
        "mcp-protocol-version",
        "mcp-session-id",
        "transfer-encoding",
    ]
    .iter()
    .any(|reserved| name.eq_ignore_ascii_case(reserved))
}

pub fn is_sensitive_http_header(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase().replace('_', "-");
    matches!(
        normalized.as_str(),
        "authorization" | "proxy-authorization" | "cookie" | "set-cookie"
    ) || [
        "token",
        "secret",
        "api-key",
        "apikey",
        "credential",
        "password",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
}

fn require_secret_ref(reference: &str, names: &HashSet<String>) -> Result<(), ManifestError> {
    if names.contains(reference) {
        Ok(())
    } else {
        Err(ManifestError::Validation(format!(
            "unknown secret reference {reference:?}"
        )))
    }
}

fn validate_command(field: &str, command: &[String]) -> Result<(), ManifestError> {
    if command.is_empty() || command.len() > 128 {
        return Err(ManifestError::Validation(format!(
            "{field} must contain 1..=128 arguments"
        )));
    }
    for value in command {
        validate_nonempty(field, value, 4096)?;
        if value.contains('\0') {
            return Err(ManifestError::Validation(format!(
                "{field} contains a NUL byte"
            )));
        }
    }
    Ok(())
}

fn validate_relative_path(field: &str, value: &str) -> Result<(), ManifestError> {
    validate_nonempty(field, value, 4096)?;
    let path = Path::new(value);
    if path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ManifestError::Validation(format!(
            "{field} must stay within the package root"
        )));
    }
    Ok(())
}

fn validate_json_schema_root(field: &str, value: &serde_json::Value) -> Result<(), ManifestError> {
    if value.is_object() || value.is_boolean() {
        Ok(())
    } else {
        Err(ManifestError::Validation(format!(
            "{field} must be a JSON Schema object or boolean"
        )))
    }
}

fn validate_http_url(field: &str, value: &str) -> Result<(), ManifestError> {
    let (scheme, host) = parsed_http_endpoint(field, value)?;
    if scheme == "https" || (scheme == "http" && is_loopback_host(&host)) {
        return Ok(());
    }
    Err(ManifestError::Validation(format!(
        "{field} must use HTTPS (loopback HTTP is allowed)"
    )))
}

fn validate_mcp_http_url(field: &str, value: &str) -> Result<(), ManifestError> {
    let (scheme, host) = parsed_http_endpoint(field, value)?;
    if scheme == "https"
        || (scheme == "http" && (is_loopback_host(&host) || is_private_network_host(&host)))
    {
        return Ok(());
    }
    Err(ManifestError::Validation(format!(
        "{field} must use HTTPS (loopback or private-network HTTP is allowed for MCP)"
    )))
}

fn parsed_http_endpoint(field: &str, value: &str) -> Result<(String, String), ManifestError> {
    let uri = value
        .parse::<http::Uri>()
        .map_err(|_| ManifestError::Validation(format!("{field} must be a valid HTTP URL")))?;
    let scheme = uri.scheme_str().ok_or_else(|| {
        ManifestError::Validation(format!("{field} must include http:// or https://"))
    })?;
    if !matches!(scheme, "http" | "https") {
        return Err(ManifestError::Validation(format!(
            "{field} must use HTTP or HTTPS"
        )));
    }
    let authority = uri
        .authority()
        .ok_or_else(|| ManifestError::Validation(format!("{field} must include a host")))?;
    if authority.as_str().contains('@') {
        return Err(ManifestError::Validation(format!(
            "{field} must not contain credentials"
        )));
    }
    Ok((scheme.to_owned(), authority.host().to_owned()))
}

fn is_loopback_host(host: &str) -> bool {
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn is_private_network_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok_and(|address| match address {
        IpAddr::V4(address) => address.is_private() || address.is_link_local(),
        IpAddr::V6(address) => address.is_unique_local() || address.is_unicast_link_local(),
    })
}

fn validate_nonempty(field: &str, value: &str, max_len: usize) -> Result<(), ManifestError> {
    if value.trim().is_empty() || value.len() > max_len {
        Err(ManifestError::Validation(format!(
            "{field} must contain 1..={max_len} bytes"
        )))
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/unit/capability_validation.rs"]
mod tests;
