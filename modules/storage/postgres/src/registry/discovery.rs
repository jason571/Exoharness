use super::{RegistryAccess, actor_valid, find, mutable, now};
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{CapabilityKind, HealthStatus, PermissionDecision};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
impl PostgresStore {
    pub fn observe_capability(
        &self,
        access: &RegistryAccess,
        id: &str,
        expected: i64,
        tools: Option<&[Value]>,
        server_info: &Value,
        actor: &str,
    ) -> Result<()> {
        actor_valid(actor)?;
        let time = now()?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let record = mutable(&mut tx, access, id, expected)?;
        if record.manifest.kind() != CapabilityKind::McpServer {
            return Err(StoreError::Invalid(
                "only MCP discovery has protocol schema evidence",
            ));
        }
        let allowed:i64=tx.query_one("SELECT COUNT(*) FROM capability_permissions WHERE capability_id=$1 AND decision=$2",&[&id,&PermissionDecision::Allow.as_str()])?.get(0);
        if allowed != record.manifest.permissions.len() as i64 {
            return Err(StoreError::NotAuthorized);
        }
        let status = if tools.is_some() {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unavailable
        };
        tx.execute("UPDATE capability_health SET status=$2,message=$3,checked_at=$4 WHERE capability_id=$1",&[&id,&status.as_str(),&if tools.is_some(){"标准MCP初始化及工具发现通过"}else{"标准MCP连接或工具发现未完成"},&time])?;
        if let Some(tools) = tools {
            if tools.len() > 1024
                || !server_info.is_object()
                || tools.iter().any(|tool| {
                    !tool.is_object() || !tool.get("inputSchema").is_some_and(Value::is_object)
                })
            {
                return Err(StoreError::Invalid("invalid MCP discovery evidence"));
            }
            let encoded = serde_json::to_string(tools)?;
            let info = serde_json::to_string(server_info)?;
            if encoded.len() > 8 * 1024 * 1024 || info.len() > 1024 * 1024 {
                return Err(StoreError::Invalid("discovery evidence exceeds limit"));
            }
            let digest = format!("{:x}", Sha256::digest(encoded.as_bytes()));
            tx.execute("INSERT INTO capability_discovery(capability_id,revision,schema_digest,tools_json,server_info_json,observed_at) VALUES($1,$2,$3,$4,$5,$6) ON CONFLICT(capability_id,revision) DO UPDATE SET schema_digest=EXCLUDED.schema_digest,tools_json=EXCLUDED.tools_json,server_info_json=EXCLUDED.server_info_json,observed_at=EXCLUDED.observed_at",&[&id,&expected,&digest,&encoded,&info,&time])?;
        }
        tx.execute("INSERT INTO registry_audit(actor,action,capability_id,details_json,created_at) VALUES($1,'capability.observed',$2,$3,$4)",&[&actor,&id,&json!({"revision":expected,"status":status,"tool_count":tools.map(|tools|tools.len())}).to_string(),&time])?;
        tx.commit()?;
        Ok(())
    }
    pub fn discovered_tools(&self, access: &RegistryAccess, id: &str) -> Result<Vec<Value>> {
        let mut connection = self.connection()?;
        let record = find(&mut *connection, access, id, false)?;
        let row=connection.query_opt("SELECT d.tools_json,d.schema_digest FROM capability_discovery d JOIN capability_health h ON h.capability_id=d.capability_id JOIN capabilities c ON c.id=d.capability_id WHERE d.capability_id=$1 AND d.revision=$2 AND c.revision=$2 AND h.status='healthy'",&[&id,&record.revision])?;
        let Some(row) = row else {
            return Ok(Vec::new());
        };
        let encoded: String = row.get(0);
        let digest: String = row.get(1);
        if format!("{:x}", Sha256::digest(encoded.as_bytes())) != digest {
            return Err(StoreError::Decode);
        }
        let value =
            exoharness_contracts::strict_json::parse(&encoded).map_err(|_| StoreError::Decode)?;
        value.as_array().cloned().ok_or(StoreError::Decode)
    }
}
