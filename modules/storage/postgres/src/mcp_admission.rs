// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Cross-session admission uses the durable invocation ledger, not an
//! in-memory semaphore. A submitted or uncertain call occupies capacity
//! until an authoritative receipt resolves it, including after a restart.
use crate::{Result, StoreError};
use postgres::Transaction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationAdmission {
    Claimed,
    Waiting,
    Unavailable,
}

pub(crate) fn has_capacity(
    tx: &mut Transaction<'_>,
    capability: &str,
    invocation: &str,
    limit: u32,
) -> Result<bool> {
    if !(1..=1024).contains(&limit) {
        return Err(StoreError::Invalid("MCP concurrency must be in 1..=1024"));
    }
    // Claims in different sessions/processes share one short transaction lock.
    // No connection or database lock is held while waiting or executing MCP.
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended(current_schema() || ':' || $1, 4271072041))",
        &[&capability],
    )?;
    let row = tx.query_one(
        "SELECT COUNT(*), COUNT(*) FILTER (WHERE outcome_status='unknown')
         FROM execution_invocations WHERE capability_id=$1 AND status='submitted'",
        &[&capability],
    )?;
    if row.get::<_, i64>(0) >= i64::from(limit) {
        if row.get::<_, i64>(1) > 0 {
            return Err(StoreError::OutcomeUnknown);
        }
        return Ok(false);
    }
    // FIFO among calls actually prepared for execution; cancelled/terminal
    // Runs cannot block the queue. Snapshot/tool discovery is not admission.
    let ahead: i64 = tx
        .query_one(
            "SELECT COUNT(*) FROM execution_invocations queued
         JOIN execution_runs r ON r.id=queued.latest_run_id
         JOIN execution_invocations mine ON mine.id=$2
         WHERE queued.capability_id=$1 AND queued.status='prepared'
           AND r.status='running'
           AND (queued.created_at,queued.id)<(mine.created_at,mine.id)",
            &[&capability, &invocation],
        )?
        .get(0);
    Ok(ahead < i64::from(limit) - row.get::<_, i64>(0))
}
