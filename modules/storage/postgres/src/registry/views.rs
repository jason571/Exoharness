// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{COLUMNS, RegistryAccess, decode};
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{
    CapabilityHealth, HealthStatus, PermissionDecision, PermissionRecord, RegisteredCapabilityView,
};
use std::collections::BTreeMap;

impl PostgresStore {
    /// Three bounded reads under one snapshot, avoiding N+1 projections and
    /// inconsistent old-manifest/new-permission combinations.
    pub fn capability_views(
        &self,
        access: &RegistryAccess,
        limit: usize,
    ) -> Result<Vec<RegisteredCapabilityView>> {
        let mut connection = self.connection()?;
        let mut tx = connection
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::RepeatableRead)
            .start()?;
        let limit = i64::try_from(limit.clamp(1, 1000))
            .map_err(|_| StoreError::Invalid("invalid list limit"))?;
        let sql = format!(
            "SELECT {COLUMNS} FROM capabilities WHERE $1::TEXT IS NULL OR scope_kind='global' OR (scope_kind='user' AND scope_key=$1) ORDER BY name,id LIMIT $2"
        );
        let records = tx
            .query(&sql, &[&access.owner(), &limit])?
            .into_iter()
            .map(decode)
            .collect::<Result<Vec<_>>>()?;
        let ids: Vec<_> = records.iter().map(|record| record.id.clone()).collect();
        let mut permissions: BTreeMap<String, Vec<PermissionRecord>> = BTreeMap::new();
        for row in tx.query("SELECT capability_id,permission,decision,decided_by,decided_at FROM capability_permissions WHERE capability_id=ANY($1) ORDER BY permission",&[&ids])? {
            let id:String=row.get(0);
            permissions.entry(id.clone()).or_default().push(PermissionRecord {capability_id:id,permission:row.get(1),decision:PermissionDecision::from_db(row.get(2)).ok_or(StoreError::Decode)?,decided_by:row.get(3),decided_at:row.get(4)});
        }
        let mut health = BTreeMap::new();
        for row in tx.query("SELECT capability_id,status,message,checked_at FROM capability_health WHERE capability_id=ANY($1)",&[&ids])? {
            let id:String=row.get(0);
            health.insert(id.clone(),CapabilityHealth {capability_id:id,status:HealthStatus::from_db(row.get(1)).ok_or(StoreError::Decode)?,message:row.get(2),checked_at:row.get(3)});
        }
        tx.commit()?;
        Ok(records
            .into_iter()
            .map(|capability| {
                let id = &capability.id;
                RegisteredCapabilityView {
                    permissions: permissions.remove(id).unwrap_or_default(),
                    health: health.remove(id),
                    capability,
                }
            })
            .collect())
    }
}
