// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::codec::{encoded, identifier, now_ms, number, parsed};
use crate::{EventEnvelope, ExecutionEvent, PostgresStore, Result, StoreError};
use postgres::Transaction;

/// Mutations acquire session -> run -> invocation locks in that order.
pub(crate) fn lock_session(tx: &mut Transaction<'_>, owner: &str, session: &str) -> Result<i64> {
    lock_session_inner(tx, owner, session, false)
}

/// Trusted adapter receipts may finish already-submitted work after the user
/// archives a conversation. This does not grant read, resume or submit access.
pub(crate) fn lock_receipt_session(
    tx: &mut Transaction<'_>,
    owner: &str,
    session: &str,
) -> Result<i64> {
    lock_session_inner(tx, owner, session, true)
}

fn lock_session_inner(
    tx: &mut Transaction<'_>,
    owner: &str,
    session: &str,
    allow_archived_receipt: bool,
) -> Result<i64> {
    identifier(owner)?;
    identifier(session)?;
    let row = tx
        .query_opt(
            "SELECT archived FROM execution_sessions WHERE id=$1 AND owner_user_id=$2 FOR UPDATE",
            &[&session, &owner],
        )?
        .ok_or(StoreError::NotFound)?;
    if row.get::<_, i64>(0) != 0 && !allow_archived_receipt {
        return Err(StoreError::Conflict);
    }
    Ok(tx
        .query_one(
            "SELECT COALESCE(SUM(last_sequence),0)::bigint FROM execution_runs WHERE session_id=$1",
            &[&session],
        )?
        .get(0))
}
pub(crate) fn append(
    tx: &mut Transaction<'_>,
    session: &str,
    run: &str,
    event: ExecutionEvent,
) -> Result<()> {
    let row=tx.query_one("UPDATE execution_runs SET last_sequence=last_sequence+1 WHERE id=$1 AND session_id=$2 RETURNING last_sequence",&[&run,&session])?;
    let seq: i64 = row.get(0);
    let label = serde_json::to_value(&event)?["type"]
        .as_str()
        .ok_or(StoreError::Decode)?
        .to_owned();
    let envelope = EventEnvelope {
        version: 1,
        session_id: session.into(),
        run_id: run.into(),
        sequence: seq as u64,
        event,
    };
    tx.execute("INSERT INTO execution_events(run_id,sequence,event_type,payload_json,created_at) VALUES($1,$2,$3,$4,$5)",&[&run,&seq,&format!("pro_{label}"),&encoded(&envelope)?,&now_ms()?])?;
    tx.execute(
        "UPDATE execution_sessions SET updated_at=$2 WHERE id=$1",
        &[&session, &now_ms()?],
    )?;
    Ok(())
}
impl PostgresStore {
    pub fn events(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        after: u64,
        limit: usize,
    ) -> Result<Vec<EventEnvelope>> {
        if !(1..=1000).contains(&limit) {
            return Err(StoreError::Invalid("event page size must be 1..1000"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        super::runs::read_run(&mut tx, session, run)?;
        let rows=tx.query("SELECT payload_json FROM execution_events WHERE run_id=$1 AND sequence>$2 ORDER BY sequence LIMIT $3",&[&run,&number(after)?,&(limit as i64)])?;
        let events = rows
            .into_iter()
            .map(|row| parsed(&row.get::<_, String>(0)))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(events)
    }
}
