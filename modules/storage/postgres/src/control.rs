use crate::codec::{encoded, now_ms, parsed};
use crate::{RecoveryDecision, Result, StoreError};
use exoharness_contracts::retry::ConsecutiveCalls;
use postgres::Transaction;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InvocationControl {
    pub logical_key: String,
    pub fingerprint: String,
    pub attempts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RunControl {
    pub version: u32,
    pub policy: Value,
    pub retry: ConsecutiveCalls,
    pub recovery: Option<RecoveryDecision>,
    pub snapshot_digest: String,
    pub invocations: BTreeMap<String, InvocationControl>,
}

/// Additive metadata inside the existing checkpoint JSON. Complete arguments
/// remain only in execution_invocations.arguments_json, never duplicated here.
pub(crate) fn load(tx: &mut Transaction<'_>, run: &str) -> Result<RunControl> {
    let row = tx
        .query_opt(
            "SELECT context_json FROM execution_context_checkpoints WHERE run_id=$1",
            &[&run],
        )?
        .ok_or(StoreError::MigrationRequired)?;
    let root: Value = parsed(&row.get::<_, String>(0))?;
    let control = root
        .get("_exoharness_pro")
        .ok_or(StoreError::MigrationRequired)?;
    let state: RunControl = serde_json::from_value(control.clone())?;
    if state.version != 1 {
        return Err(StoreError::MigrationRequired);
    }
    Ok(state)
}

pub(crate) fn save(tx: &mut Transaction<'_>, run: &str, control: &RunControl) -> Result<()> {
    let existing = tx.query_opt(
        "SELECT context_json FROM execution_context_checkpoints WHERE run_id=$1",
        &[&run],
    )?;
    let mut root: Value = if let Some(row) = existing {
        parsed(&row.get::<_, String>(0))?
    } else {
        serde_json::json!({"version":1,"messages":[],"covered_call_ids":[]})
    };
    root.as_object_mut()
        .ok_or(StoreError::Decode)?
        .insert("_exoharness_pro".into(), serde_json::to_value(control)?);
    tx.execute("INSERT INTO execution_context_checkpoints(run_id,context_json,updated_at) VALUES($1,$2,$3) ON CONFLICT(run_id) DO UPDATE SET context_json=EXCLUDED.context_json,updated_at=EXCLUDED.updated_at",&[&run,&encoded(&root)?,&now_ms()?])?;
    Ok(())
}
