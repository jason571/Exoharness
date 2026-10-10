use super::{COLUMNS, RegistryAccess, decode, find};
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{
    AuditEvent, CapabilityHealth, CapabilityVersion, HealthStatus, PermissionDecision,
    PermissionRecord, StoredCapability,
};

impl PostgresStore {
    pub fn capabilities(
        &self,
        access: &RegistryAccess,
        limit: usize,
    ) -> Result<Vec<StoredCapability>> {
        let limit = i64::try_from(limit.clamp(1, 1000))
            .map_err(|_| StoreError::Invalid("invalid list limit"))?;
        let sql = format!(
            "SELECT {COLUMNS} FROM capabilities WHERE $1::TEXT IS NULL OR scope_kind='global' OR (scope_kind='user' AND scope_key=$1) ORDER BY name,id LIMIT $2"
        );
        self.connection()?
            .query(&sql, &[&access.owner(), &limit])?
            .into_iter()
            .map(decode)
            .collect()
    }
    pub fn capability(&self, access: &RegistryAccess, id: &str) -> Result<StoredCapability> {
        find(&mut *self.connection()?, access, id, false)
    }
    pub fn capability_permissions(
        &self,
        access: &RegistryAccess,
        id: &str,
    ) -> Result<Vec<PermissionRecord>> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        find(&mut tx, access, id, false)?;
        let result=tx.query("SELECT permission,decision,decided_by,decided_at FROM capability_permissions WHERE capability_id=$1 ORDER BY permission",&[&id])?.into_iter().map(|row| {
            Ok(PermissionRecord {capability_id:id.into(),permission:row.get(0),decision:PermissionDecision::from_db(row.get(1)).ok_or(StoreError::Decode)?,decided_by:row.get(2),decided_at:row.get(3)})
        }).collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(result)
    }
    pub fn capability_health(
        &self,
        access: &RegistryAccess,
        id: &str,
    ) -> Result<Option<CapabilityHealth>> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        find(&mut tx, access, id, false)?;
        let result = tx
            .query_opt(
                "SELECT status,message,checked_at FROM capability_health WHERE capability_id=$1",
                &[&id],
            )?
            .map(|row| {
                Ok::<_, StoreError>(CapabilityHealth {
                    capability_id: id.into(),
                    status: HealthStatus::from_db(row.get(0)).ok_or(StoreError::Decode)?,
                    message: row.get(1),
                    checked_at: row.get(2),
                })
            })
            .transpose()?;
        tx.commit()?;
        Ok(result)
    }
    pub fn capability_versions(
        &self,
        access: &RegistryAccess,
        id: &str,
    ) -> Result<Vec<CapabilityVersion>> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        find(&mut tx, access, id, false)?;
        let result=tx.query("SELECT revision,version,manifest_json,manifest_digest,source_digest,created_at FROM capability_versions WHERE capability_id=$1 ORDER BY revision DESC",&[&id])?.into_iter().map(|row| {
            Ok(CapabilityVersion {capability_id:id.into(),revision:row.get(0),version:row.get(1),manifest:serde_json::from_str(row.get(2))?,manifest_digest:row.get(3),source_digest:row.get(4),created_at:row.get(5)})
        }).collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(result)
    }
    pub fn registry_audit(&self, access: &RegistryAccess, limit: usize) -> Result<Vec<AuditEvent>> {
        if !matches!(access, RegistryAccess::Administrator) {
            return Err(StoreError::NotAuthorized);
        }
        let limit = i64::try_from(limit.clamp(1, 1000))
            .map_err(|_| StoreError::Invalid("invalid audit limit"))?;
        self.connection()?.query("SELECT id,actor,action,capability_id,details_json,created_at FROM registry_audit ORDER BY id DESC LIMIT $1",&[&limit])?.into_iter().map(|row| {
            Ok(AuditEvent {id:row.get(0),actor:row.get(1),action:row.get(2),capability_id:row.get(3),details:serde_json::from_str(row.get(4))?,created_at:row.get(5)})
        }).collect()
    }
}
