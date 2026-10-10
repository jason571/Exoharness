// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Exact model input/output evidence is private journal data, not chat prose.
use crate::{
    PostgresStore, Result, RunState, StoreError,
    codec::{encoded, now_ms},
    journal::lock_session,
    runs::read_run,
};
use serde_json::{Value, json};
impl PostgresStore {
    pub fn chat_model_start(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        request: &Value,
    ) -> Result<i64> {
        self.chat_model_start_sequence(owner, session, run, 1, request)
    }
    pub fn chat_model_start_sequence(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        sequence: u32,
        request: &Value,
    ) -> Result<i64> {
        if sequence == 0 || sequence > 128 {
            return Err(StoreError::Invalid("invalid model sequence"));
        }
        if encoded(request)?.len() > 1024 * 1024 {
            return Err(StoreError::Invalid("model request too large"));
        }
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.state != RunState::Running {
            return Err(StoreError::Conflict);
        }
        let start = now_ms()?;
        let logical = format!("chat-model-{sequence}");
        let id = format!("{run}:{logical}");
        let sequence = i64::from(sequence);
        tx.execute("INSERT INTO execution_model_requests(run_id,logical_call_id,attempt,sequence,manifest_json) VALUES($1,$2,1,$3,$4)",&[&run,&logical,&sequence,&encoded(request)?])?;
        tx.execute("INSERT INTO execution_model_calls(id,run_id,logical_call_id,sequence,call_kind,attempt,provider,model,api,status,stop_reason,started_at,completed_at,duration_ms,usage_reported,usage_json,context_json,output_json) VALUES($1,$2,$7,$8,'chat',1,'',$3,$4,'submitted','',$5,$5,0,0,'{}',$6,'{}')",
            &[&id,&run,&current.snapshot.model.config_id,&current.snapshot.model.adapter,&start,&json!({"message_count":request["messages"].as_array().map_or(0,Vec::len),"tools":request["tools"].as_array().map_or(0,Vec::len)}).to_string(),&logical,&sequence])?;
        tx.commit()?;
        Ok(start)
    }
    pub fn chat_model_finish(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        response: &Value,
        error: Option<&str>,
    ) -> Result<()> {
        self.chat_model_finish_sequence(owner, session, run, 1, response, error)
    }
    pub fn chat_model_finish_sequence(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        sequence: u32,
        response: &Value,
        error: Option<&str>,
    ) -> Result<()> {
        if sequence == 0 || sequence > 128 {
            return Err(StoreError::Invalid("invalid model sequence"));
        }
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock_session(&mut tx, owner, session)?;
        read_run(&mut tx, session, run)?;
        let now = now_ms()?;
        let usage = response.get("usage").cloned().unwrap_or(json!({}));
        let reported = if usage["input_tokens"].is_u64() && usage["output_tokens"].is_u64() {
            1_i64
        } else {
            0
        };
        let output = encoded(response)?;
        if output.len() > 1024 * 1024 {
            return Err(StoreError::Invalid("model response too large"));
        }
        let id = format!("{run}:chat-model-{sequence}");
        let provider = response
            .pointer("/model/provider")
            .and_then(Value::as_str)
            .unwrap_or("");
        let status = if error.is_some() {
            "failed"
        } else {
            "succeeded"
        };
        let updated=tx.execute("UPDATE execution_model_calls SET status=$2,error=$3,provider=$4,completed_at=$5,duration_ms=GREATEST($5-started_at,0),usage_reported=$6,usage_json=$7,output_json=$8,stop_reason=$9 WHERE id=$1 AND status='submitted'",
            &[&id,&status,&error,&provider,&now,&reported,&encoded(&usage)?,&output,&if error.is_some(){"error"}else{"stop"}])?;
        if updated != 1 {
            return Err(StoreError::Conflict);
        }
        // Derive from committed model receipts rather than overwriting with
        // the latest call. The session lock makes this transaction coherent;
        // submitted calls and duplicate finishes cannot inflate usage.
        let rows = tx.query(
            "SELECT usage_json FROM execution_model_calls WHERE run_id=$1 AND status IN ('succeeded','failed') ORDER BY sequence",
            &[&run],
        )?;
        let mut aggregate = exoharness_contracts::model::Usage {
            input_tokens: Some(0),
            output_tokens: Some(0),
            cache_read_tokens: Some(0),
            cache_write_tokens: Some(0),
        };
        for row in rows {
            let value: Value = serde_json::from_str(&row.get::<_, String>(0))
                .map_err(|_| StoreError::Invalid("invalid persisted model usage"))?;
            let sum = |previous: Option<u64>, key: &str| {
                previous
                    .zip(value[key].as_u64())
                    .map(|(a, b)| a.saturating_add(b))
            };
            aggregate.input_tokens = sum(aggregate.input_tokens, "input_tokens");
            aggregate.output_tokens = sum(aggregate.output_tokens, "output_tokens");
            aggregate.cache_read_tokens = sum(aggregate.cache_read_tokens, "cache_read_tokens");
            aggregate.cache_write_tokens = sum(aggregate.cache_write_tokens, "cache_write_tokens");
        }
        let mut cumulative = serde_json::to_value(&aggregate)
            .map_err(|_| StoreError::Invalid("invalid aggregate model usage"))?;
        // Cached input is a subset of input, not an additional token charge.
        cumulative["total_tokens"] = json!(
            aggregate
                .input_tokens
                .zip(aggregate.output_tokens)
                .map(|(a, b)| a.saturating_add(b))
        );
        tx.execute(
            "UPDATE execution_run_budgets SET usage_json=$2,updated_at=$3 WHERE run_id=$1",
            &[&run, &encoded(&cumulative)?, &now],
        )?;
        tx.commit()?;
        Ok(())
    }
}
