// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Credential updates create a new immutable binding revision, not mutate a
//! credential used by a frozen Run. Old keys are retained for snapshot leases.
use crate::{Mcp, McpError, Result};
use exoharness_contracts::capability::{CapabilityKind, StoredCapability, ValidatedCapability};
use exoharness_storage_postgres::{RegistryAccess, StoreError};
impl Mcp {
    pub fn update_secret(
        &self,
        access: &RegistryAccess,
        id: &str,
        name: &str,
        value: Option<&str>,
        expected: Option<i64>,
        actor: &str,
    ) -> Result<StoredCapability> {
        let _guard = self.changes.lock().map_err(|_| McpError::Configuration)?;
        let record = self.registry.0.capability(access, id)?;
        if !matches!(access, RegistryAccess::Administrator) {
            return Err(StoreError::NotAuthorized.into());
        }
        if record.manifest.kind() != CapabilityKind::McpServer {
            return Err(McpError::Configuration);
        }
        let revision = expected.unwrap_or(record.revision);
        if revision != record.revision {
            return Err(StoreError::Conflict.into());
        }
        let mut manifest = record.manifest;
        let reference = manifest
            .secret_refs
            .iter_mut()
            .find(|r| r.name == name && r.provider == "local")
            .ok_or(StoreError::NotFound)?;
        let created = match value {
            Some(value) => Some(self.secrets.put(value).map_err(|_| McpError::Secret)?),
            None => None,
        };
        // A valid opaque but absent key makes deletion visible without dropping
        // the declared header or its requirement for authentication.
        reference.key = created.clone().unwrap_or_else(|| "0".repeat(64));
        let validated = ValidatedCapability::new(manifest, record.source_digest)
            .map_err(|_| McpError::Configuration)?;
        let result = self
            .registry
            .0
            .update_capability(access, id, validated, revision, actor);
        if matches!(
            &result,
            Err(StoreError::Conflict
                | StoreError::Invalid(_)
                | StoreError::NotAuthorized
                | StoreError::NotFound)
        ) && let Some(key) = created
        {
            let _ = self.secrets.delete(&key);
        }
        result.map_err(Into::into)
    }
}
