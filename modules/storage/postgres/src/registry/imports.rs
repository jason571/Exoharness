//! Atomic bounded imports. One bad capability cannot leave half a batch installed.
use super::{RegistryAccess, actor_valid, audit, invalidated, now, save_version};
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{StoredCapability, ValidatedCapability};
use std::collections::BTreeSet;
impl PostgresStore {
    pub fn register_capabilities(
        &self,
        access: &RegistryAccess,
        inputs: Vec<ValidatedCapability>,
        actor: &str,
        enabled: bool,
    ) -> Result<Vec<StoredCapability>> {
        actor_valid(actor)?;
        if inputs.is_empty() || inputs.len() > 64 {
            return Err(StoreError::Invalid(
                "import batch must contain 1 through 64 capabilities",
            ));
        }
        let mut ids = BTreeSet::new();
        for input in &inputs {
            if !access.can_write(input.manifest()) {
                return Err(StoreError::NotAuthorized);
            }
            if !ids.insert(input.manifest().id()) {
                return Err(StoreError::Invalid("duplicate capability in import batch"));
            }
        }
        let time = now()?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        // All identity locks use a global stable order, including absent names.
        for id in ids {
            tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072))",
                &[&id],
            )?;
            if tx
                .query_opt("SELECT id FROM capabilities WHERE id=$1", &[&id])?
                .is_some()
            {
                return Err(StoreError::Conflict);
            }
        }
        let mut records = Vec::with_capacity(inputs.len());
        for input in inputs {
            let manifest = input.manifest();
            let id = manifest.id();
            let previous:i64=tx.query_one("SELECT COALESCE(MAX((details_json::jsonb->>'revision')::BIGINT),0) FROM registry_audit WHERE capability_id=$1",&[&id])?.get(0);
            let revision = previous.checked_add(1).ok_or(StoreError::Conflict)?;
            let record = StoredCapability {
                id: id.clone(),
                manifest: manifest.clone(),
                manifest_digest: input.digest().into(),
                source_digest: input.source_digest().map(str::to_owned),
                revision,
                enabled,
                created_at: time,
                updated_at: time,
            };
            tx.execute("INSERT INTO capabilities(id,kind,name,version,scope_kind,scope_key,manifest_json,manifest_digest,source_digest,revision,enabled,created_at,updated_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$12)",&[&id,&manifest.kind().as_str(),&manifest.name,&manifest.version,&manifest.scope.kind(),&manifest.scope.key(),&serde_json::to_string(manifest)?,&record.manifest_digest,&record.source_digest,&record.revision,&i64::from(enabled),&time])?;
            for permission in &manifest.permissions {
                tx.execute(
                    "INSERT INTO capability_permissions(capability_id,permission) VALUES($1,$2)",
                    &[&id, permission],
                )?;
            }
            save_version(&mut tx, &record)?;
            invalidated(&mut tx, &id, time)?;
            audit(&mut tx, actor, "capability.registered", &id, revision, time)?;
            records.push(record);
        }
        tx.commit()?;
        Ok(records)
    }
}
