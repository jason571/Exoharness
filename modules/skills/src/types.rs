use exoharness_contracts::capability::{CapabilityManifest, CapabilityScope};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) const MAX_FILES: usize = 1024;
pub(crate) const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_TOTAL_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_INSTRUCTION_BYTES: usize = 1024 * 1024;
pub(crate) type PackageFiles = BTreeMap<PathBuf, Vec<u8>>;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageFile {
    pub path: String,
    pub content_base64: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    #[serde(default)]
    pub package_kind: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub archive_base64: Option<String>,
    #[serde(default)]
    pub files: Vec<PackageFile>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "global")]
    pub scope: CapabilityScope,
    #[serde(default)]
    pub permissions: Vec<String>,
    /// Compatibility provenance, never used as the authenticated actor.
    #[serde(default)]
    pub actor: Option<String>,
    #[serde(default)]
    pub expected_revision: Option<i64>,
}
fn default_version() -> String {
    "0.1.0".into()
}
fn global() -> CapabilityScope {
    CapabilityScope::Global
}

#[derive(Clone, Serialize)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub size: usize,
    pub digest: String,
}
pub struct Inspection {
    pub manifest: CapabilityManifest,
    pub source_digest: String,
    pub files: Vec<FileMetadata>,
    pub(crate) content: PackageFiles,
}
#[derive(Serialize)]
pub struct FileChange {
    pub path: PathBuf,
    pub change: &'static str,
    pub previous_size: Option<usize>,
    pub proposed_size: Option<usize>,
}
#[derive(Serialize)]
pub struct PermissionChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}
#[derive(Serialize)]
pub struct Preview {
    pub action: &'static str,
    pub requires_confirmation: bool,
    pub capability_id: String,
    pub name: String,
    pub current_revision: Option<i64>,
    pub current_version: Option<String>,
    pub proposed_version: String,
    pub current_digest: Option<String>,
    pub proposed_digest: String,
    pub enabled: Option<bool>,
    pub manifest_changed: bool,
    pub permission_changes: PermissionChanges,
    pub file_changes: Vec<FileChange>,
}
