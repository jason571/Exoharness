use crate::{McpError, Result};
use exoharness_contracts::capability::{
    CapabilitySpec, McpServerSpec, PermissionDecision, PermissionRecord, StoredCapability,
};
pub(crate) fn admit<'a>(
    record: &'a StoredCapability,
    permissions: &[PermissionRecord],
) -> Result<&'a McpServerSpec> {
    if !record.enabled
        || record.manifest.permissions.iter().any(|required| {
            !permissions.iter().any(|p| {
                p.capability_id == record.id
                    && p.permission == *required
                    && p.decision == PermissionDecision::Allow
            }) || permissions.iter().any(|p| {
                p.capability_id == record.id
                    && p.permission == *required
                    && p.decision == PermissionDecision::Deny
            })
        })
    {
        return Err(McpError::Permission);
    }
    let CapabilitySpec::McpServer(spec) = &record.manifest.spec else {
        return Err(McpError::Configuration);
    };
    exoharness_contracts::capability::ValidatedCapability::new(
        record.manifest.clone(),
        record.source_digest.clone(),
    )
    .map_err(|_| McpError::Configuration)?;
    Ok(spec)
}
