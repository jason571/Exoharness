// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{decode_field, row_json, usage};
use crate::{PostgresStore, Result, StoreError};
use postgres::IsolationLevel;
use serde_json::{Value, json};
impl PostgresStore {
    pub fn trace_run(&self, run: &str, after: u64, limit: usize) -> Result<Value> {
        if limit == 0 || limit > 1000 || after > i64::MAX as u64 {
            return Err(StoreError::Invalid("invalid trace event page"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        let row=tx.query_opt("SELECT (to_jsonb(r)-'model_snapshot_json'-'capability_snapshot_json')::text,t.prompt,t.ordinal FROM execution_runs r JOIN execution_turns t ON t.id=r.turn_id JOIN execution_sessions s ON s.id=r.session_id WHERE r.id=$1",&[&run])?.ok_or(StoreError::NotFound)?;
        let prompt: String = row.get(1);
        let ordinal: i64 = row.get(2);
        let mut record = row_json(row)?;
        if let Some(b)=tx.query_opt("SELECT jsonb_build_object('limits',limits_json::jsonb,'usage',usage_json::jsonb,'stop_reason',stop_reason)::text FROM execution_run_budgets WHERE run_id=$1",&[&run])? {record["budget"]=row_json(b)?;}
        let mut calls = Vec::new();
        for row in tx.query("SELECT to_jsonb(m)::text FROM execution_model_calls m WHERE run_id=$1 ORDER BY sequence",&[&run])? {
            let mut call=row_json(row)?;
            for (source,target) in [("usage_json","usage"),("context_json","context"),("output_json","output")] {decode_field(&mut call,source,target)?;}
            call["usage_reported"]=json!(call["usage_reported"].as_i64()==Some(1));
            usage(&mut call["usage"]);calls.push(call);
        }
        let mut invocations = Vec::new();
        for row in tx.query("SELECT to_jsonb(i)::text FROM execution_invocations i WHERE first_run_id=$1 OR latest_run_id=$1 ORDER BY created_at,id",&[&run])? {
            let mut invocation=row_json(row)?;
            decode_field(&mut invocation,"arguments_json","arguments")?;
            decode_field(&mut invocation,"result_json","result")?;
            // UI vocabulary differs from the durable execution-state machine.
            // The raw state remains visible and the journal is not changed.
            invocation["journal_outcome_status"] = invocation["outcome_status"].clone();
            if matches!(invocation["outcome_status"].as_str(), Some("failed_after_execution" | "rejected_before_execution")) {
                invocation["outcome_status"] = json!("failed");
            }
            invocations.push(invocation);
        }
        let mut events = Vec::new();
        for row in tx.query("SELECT to_jsonb(e)::text FROM execution_events e WHERE run_id=$1 AND sequence>$2 ORDER BY sequence LIMIT $3",&[&run,&(after as i64),&(limit as i64)])? {
            let mut event=row_json(row)?;decode_field(&mut event,"payload_json","payload")?;
            let payload=event["payload"]["event"].clone();
            if payload["type"]=="chat" {event["payload"]=payload["data"]["payload"].clone();event["event_type"]=payload["data"]["payload"]["type"].clone();}
            else {event["payload"]=payload;}
            events.push(event);
        }
        tx.commit()?;
        Ok(
            json!({"run":record,"prompt":prompt,"turn_ordinal":ordinal,"model_calls":calls,"invocations":invocations,"events":events}),
        )
    }
}
