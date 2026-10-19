// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::codec::{digest, encoded, identifier, now_ms, number, parsed, value};
use crate::journal::{append, lock_session};
use crate::runs::read_run;
use crate::{
    ExecutionEvent, InvocationIntent, InvocationRecord, PostgresStore, RecoveryAction,
    RecoveryDecision, Result, RunState, StoreError,
};
use exoharness_contracts::invocation::InvocationState;
use postgres::Transaction;
use serde_json::Value;

fn read(tx: &mut Transaction<'_>, run: &str, id: &str) -> Result<InvocationRecord> {
    let control = super::control::load(tx, run)?;
    let metadata = control.invocations.get(id).ok_or(StoreError::NotFound)?;
    let row=tx.query_opt("SELECT capability_id,capability_revision,tool_name,arguments_json,status,outcome_status,result_json FROM execution_invocations WHERE id=$1 AND latest_run_id=$2 FOR UPDATE",&[&id,&run])?.ok_or(StoreError::NotFound)?;
    let intent = InvocationIntent {
        id: id.into(),
        logical_key: metadata.logical_key.clone(),
        capability_id: row.get(0),
        capability_revision: row.get::<_, i64>(1) as u64,
        tool: row.get(2),
        arguments: parsed(&row.get::<_, String>(3))?,
    };
    if digest(&value(&intent)?)? != metadata.fingerprint {
        return Err(StoreError::Conflict);
    }
    let state = match (
        row.get::<_, String>(4).as_str(),
        row.get::<_, Option<String>>(5).as_deref(),
    ) {
        ("prepared", _) => InvocationState::Prepared,
        ("submitted", Some("unknown")) => InvocationState::OutcomeUnknown,
        ("submitted", _) => InvocationState::Submitted,
        ("completed", Some("success")) => InvocationState::Succeeded,
        ("completed", Some("rejected_before_execution")) => {
            InvocationState::RejectedBeforeExecution
        }
        ("completed", Some("failed_after_execution")) => InvocationState::FailedAfterExecution,
        _ => return Err(StoreError::Decode),
    };
    Ok(InvocationRecord {
        run_id: run.into(),
        intent,
        state,
        attempts: metadata.attempts,
        result: row
            .get::<_, Option<String>>(6)
            .map(|s| parsed(&s))
            .transpose()?,
    })
}

impl PostgresStore {
    /// A bounded continuation view of a program that actually delivered files.
    /// Original source and large tool receipts stay in the invocation journal.
    pub fn workspace_execution_recipe(
        &self,
        owner: &str,
        session: &str,
        workspace: &str,
    ) -> Result<Option<Value>> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let row = tx.query_opt(
            "WITH recent AS (SELECT id FROM execution_runs WHERE session_id=$1 ORDER BY created_at DESC,id DESC LIMIT 8)
             SELECT i.id,jsonb_build_object('runtime',i.arguments_json::jsonb->'runtime','entrypoint',i.arguments_json::jsonb->'entrypoint',
                'arguments',COALESCE(i.arguments_json::jsonb->'arguments','[]'::jsonb),'output_files',i.arguments_json::jsonb->'output_files')::text,
                i.result_json::jsonb->>'skill_id',i.result_json::jsonb->>'skill_revision',
                (SELECT f->>'sha256' FROM jsonb_array_elements(COALESCE(i.result_json::jsonb->'files','[]'::jsonb)) f
                    WHERE f->>'path'=i.arguments_json::jsonb->>'entrypoint' LIMIT 1)
             FROM execution_invocations i JOIN recent r ON r.id=i.latest_run_id
             WHERE i.capability_id='tool:native:sandbox' AND i.tool_name='sandbox_run' AND i.outcome_status='success'
               AND COALESCE(i.arguments_json::jsonb->>'workspace_id',i.result_json::jsonb->>'workspace_id')=$2
               AND i.result_json::jsonb->>'_host_execution'='true'
               AND i.result_json::jsonb#>>'{execution,status}'='completed'
               AND i.result_json::jsonb#>>'{execution,exit_code}'='0'
               AND i.result_json::jsonb#>>'{execution,cleanup_confirmed}'='true'
               AND jsonb_array_length(COALESCE(i.result_json::jsonb->'artifacts','[]'::jsonb))>0
             ORDER BY i.completed_at DESC,i.id DESC LIMIT 1", &[&session,&workspace])?;
        let result = row.map(|row| -> Result<Value> {
            Ok(serde_json::json!({"tool":"sandbox_run","source_invocation":row.get::<_,String>(0),
                "arguments":parsed::<Value>(&row.get::<_,String>(1))?,"skill_id":row.get::<_,Option<String>>(2),
                "skill_revision":row.get::<_,Option<String>>(3).and_then(|s|s.parse::<i64>().ok()),
                "entrypoint_sha256":row.get::<_,Option<String>>(4)}))
        }).transpose()?;
        tx.commit()?;
        Ok(result)
    }

    pub fn invocation(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        id: &str,
    ) -> Result<InvocationRecord> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        read_run(&mut tx, session, run)?;
        let record = read(&mut tx, run, id)?;
        tx.commit()?;
        Ok(record)
    }

    pub fn prepare_invocation(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        intent: &InvocationIntent,
    ) -> Result<InvocationRecord> {
        for id in [
            &intent.id,
            &intent.logical_key,
            &intent.capability_id,
            &intent.tool,
        ] {
            identifier(id)?;
        }
        if !intent.arguments.is_object() {
            return Err(StoreError::Invalid(
                "tool arguments must be a native JSON object",
            ));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.state != RunState::Running {
            return Err(StoreError::Conflict);
        }
        let capability = current
            .snapshot
            .capabilities
            .iter()
            .find(|cap| {
                cap.id == intent.capability_id
                    && cap.revision == intent.capability_revision
                    && cap.allowed
            })
            .ok_or(StoreError::NotAuthorized)?;
        if !capability.tool_schemas.contains_key(&intent.tool)
            || capability.tool_schemas[&intent.tool] == Value::Bool(false)
        {
            return Err(StoreError::NotAuthorized);
        }
        let fingerprint = digest(&value(intent)?)?;
        let mut control = super::control::load(&mut tx, run)?;
        if let Some((id, _)) = control
            .invocations
            .iter()
            .find(|(id, metadata)| **id == intent.id || metadata.logical_key == intent.logical_key)
        {
            let old = read(&mut tx, run, id)?;
            if old.intent != *intent {
                return Err(StoreError::Conflict);
            }
            tx.commit()?;
            return Ok(old);
        }
        let args_digest = digest(&intent.arguments)?;
        let occurrence:i64=tx.query_one("SELECT COALESCE(MAX(occurrence),0)+1 FROM execution_invocations WHERE turn_id=$1 AND capability_id=$2 AND capability_revision=$3 AND tool_name=$4 AND arguments_digest=$5",&[&current.turn_id,&intent.capability_id,&number(intent.capability_revision)?,&intent.tool,&args_digest])?.get(0);
        tx.execute("INSERT INTO execution_invocations(id,turn_id,first_run_id,latest_run_id,capability_id,capability_revision,tool_name,arguments_digest,arguments_json,occurrence,status,created_at,updated_at) VALUES($1,$2,$3,$3,$4,$5,$6,$7,$8,$9,'prepared',$10,$10)",&[&intent.id,&current.turn_id,&run,&intent.capability_id,&number(intent.capability_revision)?,&intent.tool,&args_digest,&encoded(&intent.arguments)?,&occurrence,&now_ms()?])?;
        control.invocations.insert(
            intent.id.clone(),
            super::control::InvocationControl {
                logical_key: intent.logical_key.clone(),
                fingerprint: fingerprint.clone(),
                attempts: 0,
            },
        );
        super::control::save(&mut tx, run, &control)?;
        append(
            &mut tx,
            session,
            run,
            ExecutionEvent::InvocationPrepared {
                run_id: run.into(),
                invocation_id: intent.id.clone(),
                fingerprint,
            },
        )?;
        let record = read(&mut tx, run, &intent.id)?;
        tx.commit()?;
        Ok(record)
    }

    /// true is a newly committed submission claim; false is an existing claim
    /// or terminal outcome. Only the winning worker may contact the adapter.
    pub fn claim_invocation(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        id: &str,
    ) -> Result<bool> {
        Ok(self.claim_with_capacity(owner, session, run, id, None)?
            == crate::InvocationAdmission::Claimed)
    }

    /// Atomically reserve an MCP slot and submit the invocation. Waiting
    /// consumes neither a retry attempt nor an external tool submission.
    pub fn claim_mcp_invocation(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        id: &str,
        max_concurrency: u32,
    ) -> Result<crate::InvocationAdmission> {
        self.claim_with_capacity(owner, session, run, id, Some(max_concurrency))
    }

    fn claim_with_capacity(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        id: &str,
        limit: Option<u32>,
    ) -> Result<crate::InvocationAdmission> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.state != RunState::Running {
            return Err(StoreError::Conflict);
        }
        let record = read(&mut tx, run, id)?;
        if !record.state.may_submit() {
            tx.commit()?;
            return Ok(crate::InvocationAdmission::Unavailable);
        }
        if !crate::scheduler::pool_capacity(&mut tx, &record.intent.capability_id, id)? {
            tx.commit()?;
            return Ok(crate::InvocationAdmission::Waiting);
        }
        if let Some(limit) = limit
            && !crate::mcp_admission::has_capacity(
                &mut tx,
                &record.intent.capability_id,
                id,
                limit,
            )?
        {
            tx.commit()?;
            return Ok(crate::InvocationAdmission::Waiting);
        }
        let mut control = super::control::load(&mut tx, run)?;
        if control.retry.limit() != current.snapshot.max_consecutive_identical_calls {
            return Err(StoreError::Conflict);
        }
        let tool = encoded(&(
            record.intent.capability_id.clone(),
            record.intent.capability_revision,
            record.intent.tool.clone(),
        ))?;
        // Keep only a canonical fingerprint in persistent retry metadata.
        let signature = Value::String(digest(&record.intent.arguments)?);
        if !control.retry.admit(&tool, &signature) {
            return Err(StoreError::RepeatedCall);
        }
        crate::scheduler::reserve_pool(&mut tx, &record.intent.capability_id, id)?;
        let attempts = record.attempts.checked_add(1).ok_or(StoreError::Conflict)?;
        control
            .invocations
            .get_mut(id)
            .ok_or(StoreError::NotFound)?
            .attempts = attempts;
        super::control::save(&mut tx, run, &control)?;
        tx.execute("UPDATE execution_invocations SET status='submitted',outcome_status=NULL,result_json=NULL,result_digest=NULL,submitted_at=$2,updated_at=$2,completed_at=NULL WHERE id=$1",&[&id,&now_ms()?])?;
        append(
            &mut tx,
            session,
            run,
            ExecutionEvent::InvocationSubmitted {
                run_id: run.into(),
                invocation_id: id.into(),
                attempt: attempts,
            },
        )?;
        tx.commit()?;
        Ok(crate::InvocationAdmission::Claimed)
    }

    /// Only trusted adapters/reconciliation workers supply authoritative
    /// receipts. Late receipts survive Run cancellation and never restart it.
    pub fn resolve_invocation(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        id: &str,
        next: InvocationState,
        result: Option<Value>,
    ) -> Result<InvocationRecord> {
        if matches!(next, InvocationState::Prepared | InvocationState::Submitted)
            || (next == InvocationState::Succeeded && result.is_none())
        {
            return Err(StoreError::Invalid(
                "resolution requires a terminal/unknown state and success requires a receipt",
            ));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        crate::journal::lock_receipt_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        let record = read(&mut tx, run, id)?;
        if record.state == next && record.result == result {
            tx.commit()?;
            return Ok(record);
        }
        if !matches!(
            record.state,
            InvocationState::Submitted | InvocationState::OutcomeUnknown
        ) {
            return Err(StoreError::Conflict);
        }
        let (status, outcome) = match next {
            InvocationState::Succeeded => ("completed", "success"),
            InvocationState::RejectedBeforeExecution => ("completed", "rejected_before_execution"),
            InvocationState::FailedAfterExecution => ("completed", "failed_after_execution"),
            InvocationState::OutcomeUnknown => ("submitted", "unknown"),
            _ => unreachable!(),
        };
        let result_json = result.as_ref().map(encoded).transpose()?;
        let result_digest = result.as_ref().map(digest).transpose()?;
        tx.execute("UPDATE execution_invocations SET status=$2,outcome_status=$3,result_json=$4,result_digest=$5,updated_at=$6,completed_at=$7 WHERE id=$1",&[&id,&status,&outcome,&result_json,&result_digest,&now_ms()?,&(if next==InvocationState::OutcomeUnknown {None}else {Some(now_ms()?)})])?;
        append(
            &mut tx,
            session,
            run,
            ExecutionEvent::InvocationResolved {
                run_id: run.into(),
                invocation_id: id.into(),
                state: next,
            },
        )?;
        if current.state == RunState::Paused
            && current.recovery.as_ref().is_some_and(|decision| {
                decision.action == RecoveryAction::ReconcileExternal
                    && decision.code == "outcome_unknown"
            })
        {
            let unknown:i64=tx.query_one("SELECT COUNT(*) FROM execution_invocations WHERE turn_id=$1 AND status='submitted'",&[&current.turn_id])?.get(0);
            if unknown == 0 {
                let recovery=RecoveryDecision {action:RecoveryAction::Resume,code:"external_outcome_confirmed".into(),message:"External receipts are confirmed; continue without repeating completed calls".into()};
                let mut control = super::control::load(&mut tx, run)?;
                control.recovery = Some(recovery.clone());
                super::control::save(&mut tx, run, &control)?;
                append(
                    &mut tx,
                    session,
                    run,
                    ExecutionEvent::RecoveryUpdated {
                        run_id: run.into(),
                        recovery,
                    },
                )?;
            }
        }
        let resolved = read(&mut tx, run, id)?;
        tx.commit()?;
        Ok(resolved)
    }
}
