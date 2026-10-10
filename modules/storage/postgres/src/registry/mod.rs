//! Legacy registry tables; no probing, installation or execution in this layer.
mod discovery;
mod imports;
mod reads;
mod views;
mod writes;
use crate::{PostgresStore, Result, StoreError};
use exoharness_contracts::capability::{
    CapabilityManifest, CapabilityScope, StoredCapability, manifest_digest,
};
use postgres::{GenericClient, Row, Transaction};
use serde_json::json;

#[derive(Clone, Debug)]
pub enum RegistryAccess {
    Administrator,
    User(String),
}
impl RegistryAccess {
    fn owner(&self) -> Option<&str> {
        match self {
            Self::Administrator => None,
            Self::User(id) => Some(id),
        }
    }
    fn can_write(&self, record: &CapabilityManifest) -> bool {
        match (self, &record.scope) {
            (Self::Administrator, _) => true,
            (Self::User(id), CapabilityScope::User { user_id }) => id == user_id,
            _ => false,
        }
    }
}

const COLUMNS: &str =
    "id,manifest_json,manifest_digest,source_digest,revision,enabled,created_at,updated_at";

fn decode(row: Row) -> Result<StoredCapability> {
    let manifest: CapabilityManifest = serde_json::from_str(
        row.try_get::<_, &str>("manifest_json")
            .map_err(|_| StoreError::Decode)?,
    )?;
    let enabled: i64 = row.try_get("enabled").map_err(|_| StoreError::Decode)?;
    if !matches!(enabled, 0 | 1) {
        return Err(StoreError::Decode);
    }
    let record = StoredCapability {
        id: row.try_get("id").map_err(|_| StoreError::Decode)?,
        manifest,
        manifest_digest: row
            .try_get("manifest_digest")
            .map_err(|_| StoreError::Decode)?,
        source_digest: row
            .try_get("source_digest")
            .map_err(|_| StoreError::Decode)?,
        revision: row.try_get("revision").map_err(|_| StoreError::Decode)?,
        enabled: enabled == 1,
        created_at: row.try_get("created_at").map_err(|_| StoreError::Decode)?,
        updated_at: row.try_get("updated_at").map_err(|_| StoreError::Decode)?,
    };
    if record.revision < 1
        || record.id != record.manifest.id()
        || manifest_digest(&record.manifest).map_err(|_| StoreError::Decode)?
            != record.manifest_digest
    {
        return Err(StoreError::Decode);
    }
    Ok(record)
}

fn find<C: GenericClient>(
    client: &mut C,
    access: &RegistryAccess,
    id: &str,
    lock: bool,
) -> Result<StoredCapability> {
    let sql = format!(
        "SELECT {COLUMNS} FROM capabilities WHERE id=$1 AND ($2::TEXT IS NULL OR scope_kind='global' OR (scope_kind='user' AND scope_key=$2)){}",
        if lock { " FOR UPDATE" } else { "" }
    );
    let row = client
        .query_opt(&sql, &[&id, &access.owner()])?
        .ok_or(StoreError::NotFound)?;
    decode(row)
}
fn mutable(
    tx: &mut Transaction<'_>,
    access: &RegistryAccess,
    id: &str,
    expected: i64,
) -> Result<StoredCapability> {
    let record = find(tx, access, id, true)?;
    if !access.can_write(&record.manifest) {
        return Err(StoreError::NotAuthorized);
    }
    if expected < 1 || record.revision != expected {
        return Err(StoreError::Conflict);
    }
    Ok(record)
}
fn now() -> Result<i64> {
    let value = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| StoreError::Invalid("system clock is before epoch"))?
        .as_millis();
    i64::try_from(value).map_err(|_| StoreError::Invalid("timestamp overflow"))
}
fn actor_valid(actor: &str) -> Result<()> {
    if actor.is_empty() || actor.len() > 256 || actor.chars().any(char::is_control) {
        return Err(StoreError::Invalid("invalid actor"));
    }
    Ok(())
}
fn audit(
    tx: &mut Transaction<'_>,
    actor: &str,
    action: &str,
    id: &str,
    revision: i64,
    time: i64,
) -> Result<()> {
    tx.execute("INSERT INTO registry_audit(actor,action,capability_id,details_json,created_at) VALUES($1,$2,$3,$4,$5)",&[&actor,&action,&id,&json!({"revision":revision}).to_string(),&time])?;
    Ok(())
}
fn save_version(tx: &mut Transaction<'_>, record: &StoredCapability) -> Result<()> {
    tx.execute("INSERT INTO capability_versions(capability_id,revision,version,manifest_json,manifest_digest,source_digest,created_at) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&record.id,&record.revision,&record.manifest.version,&serde_json::to_string(&record.manifest)?,&record.manifest_digest,&record.source_digest,&record.updated_at])?;
    Ok(())
}
fn invalidated(tx: &mut Transaction<'_>, id: &str, time: i64) -> Result<()> {
    tx.execute("INSERT INTO capability_health(capability_id,status,message,checked_at) VALUES($1,'unknown',NULL,$2) ON CONFLICT(capability_id) DO UPDATE SET status='unknown',message=NULL,checked_at=EXCLUDED.checked_at",&[&id,&time])?;
    Ok(())
}

impl PostgresStore {
    pub fn migrate_registry(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        tx.query_one("SELECT pg_advisory_xact_lock(4271072002)", &[])?;
        tx.batch_execute(include_str!("../../migrations/0002_legacy_registry.sql"))?;
        tx.batch_execute(include_str!(
            "../../migrations/0003_capability_discovery.sql"
        ))?;
        tx.commit()?;
        Ok(())
    }
}
