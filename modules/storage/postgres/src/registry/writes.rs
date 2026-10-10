use super::{RegistryAccess, actor_valid, audit, invalidated, mutable, now, save_version};
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{
    PermissionDecision, PermissionRecord, StoredCapability, ValidatedCapability,
};
use serde_json::json;

impl PostgresStore {
    pub fn register_capability(
        &self,
        access: &RegistryAccess,
        input: ValidatedCapability,
        actor: &str,
        enabled: bool,
    ) -> Result<StoredCapability> {
        self.register_capabilities(access, vec![input], actor, enabled)?
            .pop()
            .ok_or(StoreError::Decode)
    }
    pub fn update_capability(
        &self,
        access: &RegistryAccess,
        id: &str,
        input: ValidatedCapability,
        expected: i64,
        actor: &str,
    ) -> Result<StoredCapability> {
        actor_valid(actor)?;
        let time = now()?;
        if input.manifest().id() != id {
            return Err(StoreError::Invalid("capability identity cannot change"));
        }
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let mut record = mutable(&mut tx, access, id, expected)?;
        // A restored/imported current pointer can precede preserved history.
        // Never overwrite an immutable revision or delete history to update it.
        // The current-record lock still provides the same CAS/authorization.
        let latest: i64 = tx.query_one(
            "SELECT GREATEST($2::BIGINT,COALESCE(MAX(revision),0)) FROM capability_versions WHERE capability_id=$1",
            &[&id, &record.revision],
        )?.get(0);
        record.revision = latest.checked_add(1).ok_or(StoreError::Conflict)?;
        record.manifest = input.manifest().clone();
        record.manifest_digest = input.digest().into();
        record.source_digest = input.source_digest().map(str::to_owned);
        record.updated_at = time;
        tx.execute("UPDATE capabilities SET version=$2,manifest_json=$3,manifest_digest=$4,source_digest=$5,revision=$6,updated_at=$7 WHERE id=$1",&[&id,&record.manifest.version,&serde_json::to_string(&record.manifest)?,&record.manifest_digest,&record.source_digest,&record.revision,&time])?;
        // Existing decisions survive only for the same declared permission;
        // newly requested permissions never inherit an unrelated grant.
        tx.execute(
            "DELETE FROM capability_permissions WHERE capability_id=$1 AND NOT(permission=ANY($2))",
            &[&id, &record.manifest.permissions],
        )?;
        for permission in &record.manifest.permissions {
            tx.execute("INSERT INTO capability_permissions(capability_id,permission) VALUES($1,$2) ON CONFLICT DO NOTHING",&[&id,permission])?;
        }
        save_version(&mut tx, &record)?;
        invalidated(&mut tx, id, time)?;
        audit(
            &mut tx,
            actor,
            "capability.updated",
            id,
            record.revision,
            time,
        )?;
        tx.commit()?;
        Ok(record)
    }
    pub fn set_capability_enabled(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected: i64,
        enabled: bool,
        actor: &str,
    ) -> Result<StoredCapability> {
        actor_valid(actor)?;
        let time = now()?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let mut record = mutable(&mut tx, access, id, expected)?;
        if record.enabled == enabled {
            return Ok(record);
        }
        record.revision = record.revision.checked_add(1).ok_or(StoreError::Conflict)?;
        record.enabled = enabled;
        record.updated_at = time;
        tx.execute(
            "UPDATE capabilities SET enabled=$2,revision=$3,updated_at=$4 WHERE id=$1",
            &[&id, &i64::from(enabled), &record.revision, &time],
        )?;
        save_version(&mut tx, &record)?;
        invalidated(&mut tx, id, time)?;
        audit(
            &mut tx,
            actor,
            if enabled {
                "capability.enabled"
            } else {
                "capability.disabled"
            },
            id,
            record.revision,
            time,
        )?;
        tx.commit()?;
        Ok(record)
    }
    pub fn decide_capability_permission(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected: i64,
        permission: &str,
        decision: PermissionDecision,
        actor: &str,
    ) -> Result<PermissionRecord> {
        actor_valid(actor)?;
        // User installation authority never includes self-granting execution
        // permissions. Only the separate trusted administrator can decide.
        if !matches!(access, RegistryAccess::Administrator) {
            return Err(StoreError::NotAuthorized);
        }
        let time = now()?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let record = mutable(&mut tx, access, id, expected)?;
        if !record.manifest.permissions.iter().any(|p| p == permission) {
            return Err(StoreError::Invalid(
                "permission is not declared by capability",
            ));
        }
        if tx.execute("UPDATE capability_permissions SET decision=$3,decided_by=$4,decided_at=$5 WHERE capability_id=$1 AND permission=$2",&[&id,&permission,&decision.as_str(),&actor,&time])?!=1 {return Err(StoreError::NotFound);}
        invalidated(&mut tx, id, time)?;
        tx.execute("INSERT INTO registry_audit(actor,action,capability_id,details_json,created_at) VALUES($1,'permission.decided',$2,$3,$4)",&[&actor,&id,&json!({"revision":record.revision,"permission":permission,"decision":decision}).to_string(),&time])?;
        tx.commit()?;
        Ok(PermissionRecord {
            capability_id: id.into(),
            permission: permission.into(),
            decision,
            decided_by: Some(actor.into()),
            decided_at: Some(time),
        })
    }
    pub fn remove_capability(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected: i64,
        actor: &str,
    ) -> Result<()> {
        actor_valid(actor)?;
        let time = now()?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072))",
            &[&id],
        )?;
        let record = mutable(&mut tx, access, id, expected)?;
        audit(
            &mut tx,
            actor,
            "capability.removed",
            id,
            record.revision,
            time,
        )?;
        tx.execute("DELETE FROM capabilities WHERE id=$1", &[&id])?;
        tx.commit()?;
        Ok(())
    }
}
