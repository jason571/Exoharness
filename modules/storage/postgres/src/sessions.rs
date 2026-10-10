// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::codec::{digest, encoded, identifier, now_ms, number, value};
use crate::journal::{append, lock_session};
use crate::runs::read_run;
use crate::{
    ExecutionEvent, ExecutionSnapshot, PostgresStore, Result, RunRecord, SessionSummary, StoreError,
};
use exoharness_contracts::retry::ConsecutiveCalls;
use std::collections::{BTreeMap, BTreeSet};

pub struct CreateTurn<'a> {
    pub owner: &'a str,
    pub session_id: &'a str,
    pub expected_session_version: u64,
    pub turn_id: &'a str,
    pub run_id: &'a str,
    pub input: &'a str,
    pub snapshot: &'a ExecutionSnapshot,
}
pub(crate) fn validate_snapshot(snapshot: &ExecutionSnapshot) -> Result<()> {
    if snapshot.version != 1 {
        return Err(StoreError::Invalid(
            "unsupported execution snapshot version",
        ));
    }
    identifier(&snapshot.assistant_id)?;
    identifier(&snapshot.model.config_id)?;
    identifier(&snapshot.model.adapter)?;
    if !snapshot.model.settings.is_object() {
        return Err(StoreError::Invalid("model settings must be an object"));
    }
    if snapshot.max_consecutive_identical_calls == 0
        || snapshot.max_consecutive_identical_calls > 100
    {
        return Err(StoreError::Invalid(
            "consecutive identical call limit must be 1..100",
        ));
    }
    let mut ids = BTreeSet::new();
    for capability in &snapshot.capabilities {
        identifier(&capability.id)?;
        if !ids.insert(&capability.id) {
            return Err(StoreError::Invalid("duplicate capability in snapshot"));
        }
        for (tool, schema) in &capability.tool_schemas {
            identifier(tool)?;
            if !schema.is_object() && !schema.is_boolean() {
                return Err(StoreError::Invalid(
                    "JSON Schema must be an object or boolean",
                ));
            }
        }
    }
    Ok(())
}
impl PostgresStore {
    /// Trusted management callers alone may pass None to inspect all owners.
    /// HTTP/Agent boundaries must supply the authenticated principal otherwise.
    pub fn sessions(&self, owner: Option<&str>, limit: usize) -> Result<Vec<SessionSummary>> {
        if !(1..=1000).contains(&limit) {
            return Err(StoreError::Invalid("session page size must be 1..1000"));
        }
        let rows = self.connection()?.query("SELECT s.id,s.owner_user_id,s.title,s.updated_at,COALESCE((SELECT SUM(last_sequence)::bigint FROM execution_runs WHERE session_id=s.id),0) FROM execution_sessions s WHERE s.archived=0 AND ($1::text IS NULL OR s.owner_user_id=$1) ORDER BY s.updated_at DESC,s.id LIMIT $2", &[&owner,&(limit as i64)])?;
        Ok(rows
            .into_iter()
            .map(|row| SessionSummary {
                id: row.get(0),
                owner: row.get(1),
                title: row.get(2),
                updated_at: row.get(3),
                version: row.get::<_, i64>(4) as u64,
            })
            .collect())
    }

    pub fn session_runs(&self, owner: &str, session: &str, limit: usize) -> Result<Vec<RunRecord>> {
        if !(1..=1000).contains(&limit) {
            return Err(StoreError::Invalid("run page size must be 1..1000"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let ids:Vec<String> = tx.query("SELECT id FROM execution_runs WHERE session_id=$1 ORDER BY created_at DESC,id LIMIT $2", &[&session,&(limit as i64)])?.into_iter().map(|row|row.get(0)).collect();
        let runs = ids
            .iter()
            .map(|id| read_run(&mut tx, session, id))
            .collect::<Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(runs)
    }

    pub fn create_session(&self, owner: &str, session: &str) -> Result<()> {
        identifier(owner)?;
        identifier(session)?;
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        tx.execute("INSERT INTO execution_sessions(id,title,title_source,created_at,updated_at,owner_user_id) VALUES($1,'New conversation','placeholder',$3,$3,$2) ON CONFLICT DO NOTHING",&[&session,&owner,&now_ms()?])?;
        lock_session(&mut tx, owner, session)?;
        tx.commit()?;
        Ok(())
    }
    pub fn session_version(&self, owner: &str, session: &str) -> Result<u64> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let version = lock_session(&mut tx, owner, session)? as u64;
        tx.commit()?;
        Ok(version)
    }
    pub fn create_turn(&self, c: CreateTurn<'_>) -> Result<RunRecord> {
        identifier(c.turn_id)?;
        identifier(c.run_id)?;
        validate_snapshot(c.snapshot)?;
        if c.input.trim().is_empty() {
            return Err(StoreError::Invalid("turn input must not be blank"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let sequence = lock_session(&mut tx, c.owner, c.session_id)?;
        if let Some(existing) = tx.query_opt(
            "SELECT prompt FROM execution_turns WHERE id=$1 AND session_id=$2",
            &[&c.turn_id, &c.session_id],
        )? {
            let run = read_run(&mut tx, c.session_id, c.run_id)?;
            if existing.get::<_, String>(0) != c.input
                || run.turn_id != c.turn_id
                || run.snapshot != *c.snapshot
            {
                return Err(StoreError::Conflict);
            }
            tx.commit()?;
            return Ok(run);
        }
        if sequence != number(c.expected_session_version)? {
            return Err(StoreError::Conflict);
        }
        if tx.query_opt("SELECT id FROM execution_runs WHERE session_id=$1 AND status IN ('queued','pending','running','waiting_authorization')",&[&c.session_id])?.is_some() {return Err(StoreError::Conflict);}
        let ordinal: i64 = tx
            .query_one(
                "SELECT COALESCE(MAX(ordinal),0)+1 FROM execution_turns WHERE session_id=$1",
                &[&c.session_id],
            )?
            .get(0);
        let hash = digest(&value(c.snapshot)?)?;
        if ordinal == 1 {
            super::conversations::titles::first_turn(&mut tx, c.owner, c.session_id, c.input)?;
        }
        let guard = encoded(
            &ConsecutiveCalls::new(c.snapshot.max_consecutive_identical_calls)
                .map_err(StoreError::Invalid)?,
        )?;
        let cap_ids = c
            .snapshot
            .capabilities
            .iter()
            .map(|cap| &cap.id)
            .collect::<Vec<_>>();
        tx.execute("INSERT INTO execution_turns(id,session_id,ordinal,prompt,history_content,selected_model,attachments_json,capability_ids_json,status,created_at,updated_at) VALUES($1,$2,$3,$4,$4,$5,'[]',$6,'queued',$7,$7)",&[&c.turn_id,&c.session_id,&ordinal,&c.input,&c.snapshot.model.config_id,&encoded(&cap_ids)?,&now_ms()?])?;
        tx.execute("INSERT INTO execution_runs(id,session_id,turn_id,attempt,status,actor,created_at,model_snapshot_json,capability_snapshot_json) VALUES($1,$2,$3,1,'queued',$4,$5,$6,$7)",&[&c.run_id,&c.session_id,&c.turn_id,&c.owner,&now_ms()?,&encoded(&c.snapshot.model)?,&encoded(&c.snapshot.capabilities)?])?;
        tx.execute("INSERT INTO execution_run_agent_profiles(run_id,profile_snapshot_json) VALUES($1,$2)",&[&c.run_id,&serde_json::json!({"id":c.snapshot.assistant_id,"revision":c.snapshot.assistant_revision}).to_string()])?;
        let policy = serde_json::json!({"version":c.snapshot.version,"requires_execution":c.snapshot.requires_execution,"max_consecutive_identical_calls":c.snapshot.max_consecutive_identical_calls});
        super::control::save(
            &mut tx,
            c.run_id,
            &super::control::RunControl {
                version: 1,
                policy,
                retry: super::codec::parsed(&guard)?,
                recovery: None,
                snapshot_digest: hash.clone(),
                invocations: BTreeMap::new(),
            },
        )?;
        tx.execute("INSERT INTO execution_run_budgets(run_id,limits_json,usage_json,created_at,updated_at) VALUES($1,'{}','{}',$2,$2)",&[&c.run_id,&now_ms()?])?;
        append(
            &mut tx,
            c.session_id,
            c.run_id,
            ExecutionEvent::TurnCreated {
                turn_id: c.turn_id.into(),
                run_id: c.run_id.into(),
                input: c.input.into(),
            },
        )?;
        append(
            &mut tx,
            c.session_id,
            c.run_id,
            ExecutionEvent::RunCreated {
                run_id: c.run_id.into(),
                snapshot_digest: hash,
            },
        )?;
        let run = read_run(&mut tx, c.session_id, c.run_id)?;
        tx.commit()?;
        Ok(run)
    }
}
