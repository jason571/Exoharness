// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Legacy-compatible capability types; pure validation has no runtime effects.
mod model;
mod records;
mod validation;
pub use model::*;
pub use records::*;
pub use validation::{
    is_sensitive_http_header, manifest_digest, normalize_source_digest,
    validate_and_normalize_manifest,
};

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid capability manifest: {0}")]
    Validation(String),
    #[error("capability JSON is invalid")]
    Json(#[from] serde_json::Error),
}

/// Only checked manifests enter the persistence boundary. Validation is not
/// authorization or proof that an executable runtime is ready.
pub struct ValidatedCapability {
    manifest: CapabilityManifest,
    source_digest: Option<String>,
    digest: String,
}
impl ValidatedCapability {
    pub fn new(
        manifest: CapabilityManifest,
        source_digest: Option<String>,
    ) -> Result<Self, ManifestError> {
        let manifest = validate_and_normalize_manifest(manifest)?;
        let source_digest = normalize_source_digest(source_digest)?;
        let digest = manifest_digest(&manifest)?;
        Ok(Self {
            manifest,
            source_digest,
            digest,
        })
    }
    pub fn manifest(&self) -> &CapabilityManifest {
        &self.manifest
    }
    pub fn source_digest(&self) -> Option<&str> {
        self.source_digest.as_deref()
    }
    pub fn digest(&self) -> &str {
        &self.digest
    }
}
