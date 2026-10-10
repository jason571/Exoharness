// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    ImportRequest, Inspection, Preview, Result, SkillError, decode, errors::validation, preview,
    storage::Packages,
};
use exoharness_contracts::capability::{
    CapabilityKind, CapabilityScope, StoredCapability, ValidatedCapability,
};
use exoharness_core::services::Service;
use exoharness_registry::Registry;
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct Skills {
    pub(crate) registry: Arc<Registry>,
    packages: Packages,
    lifecycle: Mutex<()>,
}
impl Service for Skills {
    const KEY: &'static str = "skills";
}
impl Skills {
    pub fn open(root: PathBuf, registry: Arc<Registry>) -> Result<Self> {
        let packages = Packages::open(root)?;
        packages.recover_removals(&registry)?;
        Ok(Self {
            registry,
            packages,
            lifecycle: Mutex::new(()),
        })
    }
    fn current(&self, access: &RegistryAccess, id: &str) -> Result<Option<StoredCapability>> {
        match self.registry.0.capability(access, id) {
            Ok(record) => Ok(Some(record)),
            Err(StoreError::NotFound) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn installed(&self, record: &StoredCapability) -> Result<crate::types::PackageFiles> {
        if record.manifest.kind() != CapabilityKind::Skill {
            return Err(validation("这不是 Skill"));
        }
        self.packages.read(
            &record.id,
            record
                .source_digest
                .as_deref()
                .ok_or_else(|| validation("Skill 缺少持久化包"))?,
        )
    }
    pub fn preview(&self, access: &RegistryAccess, request: &ImportRequest) -> Result<Preview> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        authorize_scope(access, &request.scope)?;
        let inspected = decode(request)?;
        let current = self.current(access, &inspected.manifest.id())?;
        let existing = current.as_ref().map(|r| self.installed(r)).transpose()?;
        preview::build(&inspected, current.as_ref(), existing.as_ref())
    }
    pub fn import(
        &self,
        access: &RegistryAccess,
        request: &ImportRequest,
        actor: &str,
    ) -> Result<StoredCapability> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        authorize_scope(access, &request.scope)?;
        let inspected = decode(request)?;
        let id = inspected.manifest.id();
        let current = self.current(access, &id)?;
        if let Some(record) = &current {
            if record.manifest == inspected.manifest
                && record.source_digest.as_deref() == Some(&inspected.source_digest)
            {
                if request
                    .expected_revision
                    .is_some_and(|expected| expected != record.revision)
                {
                    return Err(StoreError::Conflict.into());
                }
                self.installed(record)?;
                return Ok(record.clone());
            }
            if request.expected_revision != Some(record.revision) {
                return Err(StoreError::Conflict.into());
            }
        } else if request.expected_revision.is_some() {
            return Err(StoreError::Conflict.into());
        }
        self.packages.install(&inspected)?;
        self.commit(access, inspected, current.as_ref(), actor)
    }
    fn commit(
        &self,
        access: &RegistryAccess,
        inspected: Inspection,
        current: Option<&StoredCapability>,
        actor: &str,
    ) -> Result<StoredCapability> {
        let input = ValidatedCapability::new(inspected.manifest, Some(inspected.source_digest))
            .map_err(|e| validation(e.to_string()))?;
        match current {
            Some(record) => Ok(self.registry.0.update_capability(
                access,
                &record.id,
                input,
                record.revision,
                actor,
            )?),
            None => match self
                .registry
                .0
                .register_capability(access, input, actor, false)
            {
                Ok(record) => Ok(record),
                Err(error) => Err(SkillError::Store(error)),
            },
        }
    }
    pub fn instructions(&self, access: &RegistryAccess, id: &str) -> Result<String> {
        self.instruction_view(access, id)
            .map(|(_, content)| content)
    }
    pub fn instruction_view(
        &self,
        access: &RegistryAccess,
        id: &str,
    ) -> Result<(StoredCapability, String)> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        let record = self.registry.0.capability(access, id)?;
        let files = self.installed(&record)?;
        let bytes = files
            .get(Path::new("SKILL.md"))
            .ok_or_else(|| validation("Skill 缺少 SKILL.md"))?;
        let content = std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| validation("SKILL.md 编码无效"))?;
        Ok((record, content))
    }
    pub fn files(
        &self,
        access: &RegistryAccess,
        id: &str,
    ) -> Result<Vec<crate::types::FileMetadata>> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        let record = self.registry.0.capability(access, id)?;
        let files = self.installed(&record)?;
        use sha2::{Digest, Sha256};
        Ok(files
            .iter()
            .map(|(path, bytes)| crate::types::FileMetadata {
                path: path.clone(),
                size: bytes.len(),
                digest: format!("{:x}", Sha256::digest(bytes)),
            })
            .collect())
    }
    /// One verified immutable package and registry revision under the import
    /// lease. A consumer cannot ask this method to read an arbitrary host path.
    pub fn package_snapshot(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected_revision: i64,
    ) -> Result<crate::PackageSnapshot> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        let capability = self.registry.0.capability(access, id)?;
        if capability.revision != expected_revision {
            return Err(StoreError::Conflict.into());
        }
        let files = self.installed(&capability)?;
        Ok(crate::PackageSnapshot {
            capability,
            files: files
                .into_iter()
                .map(|(path, bytes)| {
                    Ok((
                        path.to_str()
                            .ok_or_else(|| validation("Skill 路径编码无效"))?
                            .to_owned(),
                        bytes,
                    ))
                })
                .collect::<Result<_>>()?,
        })
    }
    pub fn remove(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected: i64,
        actor: &str,
    ) -> Result<()> {
        let _lease = self.lifecycle.lock().map_err(|_| SkillError::Io)?;
        let record = self.registry.0.capability(access, id)?;
        authorize_scope(access, &record.manifest.scope)?;
        if record.revision != expected {
            return Err(StoreError::Conflict.into());
        }
        if record.manifest.kind() != CapabilityKind::Skill {
            return Err(validation("这不是 Skill"));
        }
        let removal = self.packages.prepare_removal(id)?;
        self.registry
            .0
            .remove_capability(access, id, expected, actor)?;
        removal.commit()
    }
}
fn authorize_scope(access: &RegistryAccess, scope: &CapabilityScope) -> Result<()> {
    match (access, scope) {
        (RegistryAccess::Administrator, _) => Ok(()),
        (RegistryAccess::User(id), CapabilityScope::User { user_id }) if id == user_id => Ok(()),
        _ => Err(StoreError::NotAuthorized.into()),
    }
}
