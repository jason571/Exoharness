// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Host recovery references durable task ownership, including archived sessions.
use crate::{ExecutionEvent, ExecutionSnapshot, PostgresStore, Result, StoreError};
use serde_json::{Value, json};

#[derive(Clone, Debug)]
pub struct PendingMcpTask {
    pub owner: String,
    pub session: String,
    pub run: String,
    pub invocation: String,
    pub capability: String,
    pub revision: u64,
    pub snapshot: ExecutionSnapshot,
    pub task: Value,
}

impl PostgresStore {
    /// Trusted host-only enumeration, not a user-facing conversation API.
    /// Keyset paging bounds work and lets unavailable jobs yield to other jobs.
    pub fn pending_mcp_tasks(&self, after: &str, limit: usize) -> Result<Vec<PendingMcpTask>> {
        if !(1..=64).contains(&limit) {
            return Err(StoreError::Invalid(
                "reconciliation page size must be 1..64",
            ));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let rows = tx.query(
            "SELECT i.id,i.capability_id,i.capability_revision,r.id,r.session_id,s.owner_user_id,
                    evidence.payload_json::jsonb #> '{event,data,payload,task}'
             FROM execution_invocations i
             JOIN execution_runs r ON r.id=i.latest_run_id
             JOIN execution_sessions s ON s.id=r.session_id
             JOIN LATERAL (
                 SELECT e.payload_json FROM execution_events e
                 WHERE e.run_id=r.id AND e.event_type='pro_chat'
                   AND e.payload_json::jsonb #>> '{event,data,payload,type}'='mcp_background_task'
                   AND e.payload_json::jsonb #>> '{event,data,payload,tool_call_id}'=i.id
                   AND e.payload_json::jsonb #>> '{event,data,payload,capability_id}'=i.capability_id
                 ORDER BY e.sequence ASC LIMIT 1
             ) evidence ON TRUE
             WHERE i.status='submitted' AND r.status IN ('cancelled','failed','recoverable','succeeded')
               AND i.id>$1 AND s.owner_user_id IS NOT NULL
             ORDER BY i.id LIMIT $2",
            &[&after, &(limit as i64)],
        )?;
        let mut tasks = Vec::new();
        for row in rows {
            let run: String = row.get(3);
            let session: String = row.get(4);
            let snapshot = crate::runs::read_run(&mut tx, &session, &run)?.snapshot;
            tasks.push(PendingMcpTask {
                owner: row.get(5),
                session,
                run,
                invocation: row.get(0),
                capability: row.get(1),
                revision: row.get::<_, i64>(2) as u64,
                snapshot,
                task: row.get(6),
            });
        }
        tx.commit()?;
        Ok(tasks)
    }

    /// Late evidence never reopens the Run; unchanged poll states are deduplicated.
    pub fn note_mcp_reconciliation(&self, task: &PendingMcpTask, state: &str) -> Result<()> {
        if !matches!(state, "pending" | "verified" | "unavailable" | "blocked") {
            return Err(StoreError::Invalid("invalid reconciliation state"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        crate::journal::lock_receipt_session(&mut tx, &task.owner, &task.session)?;
        crate::runs::read_run(&mut tx, &task.session, &task.run)?;
        let payload = json!({"type":"mcp_background_reconciliation", "tool_call_id":task.invocation,
            "capability_id":task.capability,"task_id":task.task["id"],"state":state});
        let previous = tx.query_opt(
            "SELECT payload_json::jsonb #> '{event,data,payload}' FROM execution_events
             WHERE run_id=$1 AND event_type='pro_chat'
               AND payload_json::jsonb #>> '{event,data,payload,type}'='mcp_background_reconciliation'
               AND payload_json::jsonb #>> '{event,data,payload,tool_call_id}'=$2
             ORDER BY sequence DESC LIMIT 1", &[&task.run,&task.invocation])?;
        if previous.is_none_or(|row| row.get::<_, Value>(0) != payload) {
            crate::journal::append(
                &mut tx,
                &task.session,
                &task.run,
                ExecutionEvent::Chat { payload },
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}
