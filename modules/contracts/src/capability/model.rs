// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

/// Stable registry identity in the form `kind:scope-kind:scope-key:name`.
pub type CapabilityId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    Skill,
    McpServer,
    ToolNode,
}

impl CapabilityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Skill => "skill",
            Self::McpServer => "mcp_server",
            Self::ToolNode => "tool_node",
        }
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilityScope {
    Global,
    User { user_id: String },
    Project { project: String },
    Session { session_id: String },
}

impl CapabilityScope {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::User { .. } => "user",
            Self::Project { .. } => "project",
            Self::Session { .. } => "session",
        }
    }

    pub fn key(&self) -> &str {
        match self {
            Self::Global => "_",
            Self::User { user_id } => user_id,
            Self::Project { project } => project,
            Self::Session { session_id } => session_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CapabilitySource {
    Local {
        path: String,
    },
    /// Content or configuration uploaded through the control plane. The file
    /// name is provenance only and never interpreted as a server path.
    Uploaded {
        file_name: String,
    },
    Git {
        url: String,
        revision: String,
    },
    Remote {
        url: String,
    },
    Registry {
        registry: String,
        package: String,
        version: String,
    },
}

/// A reference to a secret held by an external secret provider.
///
/// This type intentionally has no field capable of holding the secret value.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SecretReference {
    /// Logical name used by the manifest, for example `github_token`.
    pub name: String,
    /// Provider name, for example `environment`, `keyring`, or `vault`.
    pub provider: String,
    /// Provider-specific lookup key. This is an identifier, not a credential.
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EnvironmentBinding {
    Literal { value: String },
    SecretRef { reference: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SkillSpec {
    #[serde(default = "default_skill_entrypoint")]
    pub entrypoint: String,
    /// Root within the immutable installed package.
    #[serde(default = "default_dot")]
    pub content_root: String,
    /// Skills never gain script execution implicitly. A script must be
    /// registered independently as a Tool Node before it can run.
    #[serde(default)]
    pub executable_scripts: bool,
}

fn default_skill_entrypoint() -> String {
    "SKILL.md".to_string()
}

fn default_dot() -> String {
    ".".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerSpec {
    pub namespace: String,
    pub transport: McpTransport,
    /// Optional protocol version pin. `None` means negotiate a supported
    /// version during discovery.
    #[serde(default)]
    pub protocol_version: Option<String>,
    /// Harness-side admission control for MCP calls. GPU-backed services
    /// typically set max_concurrency to the number of jobs their allocated
    /// devices can safely run at once; excess calls wait in the durable Run.
    #[serde(default)]
    pub limits: ResourceLimits,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpTransport {
    Stdio {
        command: Vec<String>,
        #[serde(default)]
        environment: BTreeMap<String, EnvironmentBinding>,
        /// Optional process working directory. Uploaded MCP packages use
        /// their immutable install root so relative resources resolve
        /// without relying on the Harness process directory.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        working_directory: Option<String>,
    },
    StreamableHttp {
        url: String,
        #[serde(default)]
        auth_secret: Option<String>,
        /// Additional request headers. Secret-backed values are resolved only
        /// when the client connects and are never stored in the manifest.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, EnvironmentBinding>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolNodeSpec {
    pub tools: Vec<ToolDefinition>,
    pub runner: RunnerSpec,
    #[serde(default)]
    pub limits: ResourceLimits,
    #[serde(default)]
    pub sandbox: SandboxRequirement,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunnerSpec {
    Process {
        command: Vec<String>,
        #[serde(default)]
        environment: BTreeMap<String, EnvironmentBinding>,
        #[serde(default)]
        working_directory: Option<String>,
    },
    Oci {
        image: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Wasm {
        module: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLimits {
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default = "default_output_bytes")]
    pub max_output_bytes: u64,
    #[serde(default = "default_concurrency")]
    pub max_concurrency: u32,
    #[serde(default)]
    pub memory_bytes: Option<u64>,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            timeout_ms: default_timeout_ms(),
            max_output_bytes: default_output_bytes(),
            max_concurrency: default_concurrency(),
            memory_bytes: None,
        }
    }
}

fn default_timeout_ms() -> u64 {
    120_000
}

fn default_output_bytes() -> u64 {
    1_048_576
}

fn default_concurrency() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SandboxRequirement {
    /// No verified isolation. Suitable only for trusted local development.
    #[default]
    None,
    /// OS-level restrictions such as namespaces, Landlock, or AppContainer.
    Os,
    /// OCI container with explicit mounts, resources, and egress policy.
    Oci,
    /// WASM component with explicitly granted WASI capabilities.
    Wasm,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "config", rename_all = "snake_case")]
pub enum CapabilitySpec {
    Skill(SkillSpec),
    McpServer(McpServerSpec),
    ToolNode(ToolNodeSpec),
}

impl CapabilitySpec {
    pub fn kind(&self) -> CapabilityKind {
        match self {
            Self::Skill(_) => CapabilityKind::Skill,
            Self::McpServer(_) => CapabilityKind::McpServer,
            Self::ToolNode(_) => CapabilityKind::ToolNode,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityManifest {
    pub schema_version: u32,
    pub name: String,
    pub version: String,
    pub description: String,
    pub scope: CapabilityScope,
    pub source: CapabilitySource,
    #[serde(default)]
    pub permissions: Vec<String>,
    #[serde(default)]
    pub secret_refs: Vec<SecretReference>,
    pub spec: CapabilitySpec,
}

impl CapabilityManifest {
    pub fn kind(&self) -> CapabilityKind {
        self.spec.kind()
    }

    /// Stable identity across manifest versions.
    pub fn id(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.kind(),
            self.scope.kind(),
            self.scope.key(),
            self.name
        )
    }
}
