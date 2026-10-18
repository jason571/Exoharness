// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Durable chat projection; no provider or background process belongs in storage.
use crate::{
    ExecutionEvent, PostgresStore, Result, RunRecord, RunState, StoreError,
    codec::{encoded, now_ms},
    journal::{append, lock_session},
    runs::read_run,
};
use serde_json::{Value, json};

impl PostgresStore {
    /// Persist chat input references, never media Base64, under the owning Run.
    pub fn save_chat_attachments(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        attachments: &Value,
    ) -> Result<()> {
        let entries = attachments
            .as_array()
            .ok_or(StoreError::Invalid("attachment list required"))?;
        if entries.len() > 8
            || entries
                .iter()
                .any(|v| !v.is_object() || v.get("data").is_some() || v.get("base64").is_some())
        {
            return Err(StoreError::Invalid(
                "attachments must contain references only",
            ));
        }
        let raw = encoded(attachments)?;
        if raw.len() > 4 * 1024 * 1024 + 64 * 1024 {
            return Err(StoreError::Invalid("attachment metadata exceeds bound"));
        }
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.state != RunState::Queued {
            return Err(StoreError::Conflict);
        }
        tx.execute(
            "UPDATE execution_turns SET attachments_json=$2 WHERE id=$1",
            &[&current.turn_id, &raw],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn chat_agent_cursor(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        metadata: &Value,
    ) -> Result<()> {
        if encoded(metadata)?.len() > 64 * 1024 || metadata.get("entries").is_some() {
            return Err(StoreError::Invalid(
                "agent cursor must reference exact journal data",
            ));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        if read_run(&mut tx, session, run)?.state != RunState::Running {
            return Err(StoreError::Conflict);
        }
        let row = tx.query_one(
            "SELECT context_json FROM execution_context_checkpoints WHERE run_id=$1",
            &[&run],
        )?;
        let mut root: Value = super::codec::parsed(&row.get::<_, String>(0))?;
        root.as_object_mut()
            .ok_or(StoreError::Decode)?
            .insert("_agent_loop".into(), metadata.clone());
        tx.execute("UPDATE execution_context_checkpoints SET context_json=$2,updated_at=$3 WHERE run_id=$1", &[&run,&encoded(&root)?,&now_ms()?])?;
        tx.commit()?;
        Ok(())
    }
    pub fn owned_run(&self, owner: &str, run: &str) -> Result<RunRecord> {
        let session:String=self.connection()?.query_opt(
            "SELECT r.session_id FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id WHERE r.id=$1 AND s.owner_user_id=$2 AND s.archived=0",
            &[&run,&owner])?.ok_or(StoreError::NotFound)?.get(0);
        self.run(owner, &session, run)
    }
    pub fn chat_event(&self, owner: &str, session: &str, run: &str, payload: Value) -> Result<()> {
        if encoded(&payload)?.len() > 1024 * 1024 {
            return Err(StoreError::Invalid("chat event exceeds bound"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if !current.state.active() {
            return Err(StoreError::Conflict);
        }
        append(&mut tx, session, run, ExecutionEvent::Chat { payload })?;
        tx.commit()?;
        Ok(())
    }
    /// Answer, totals, terminal event and Run/Turn status commit atomically.
    /// Late provider completion cannot resurrect a cancelled Run.
    pub fn finish_chat(
        &self,
        owner: &str,
        session: &str,
        run: &str,
        response: Value,
        error: Option<&str>,
    ) -> Result<()> {
        if encoded(&response)?.len() > 1024 * 1024 {
            return Err(StoreError::Invalid("chat response exceeds bound"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, session)?;
        let current = read_run(&mut tx, session, run)?;
        if current.state != RunState::Running
            && !(error.is_some() && current.state == RunState::Queued)
        {
            return Err(StoreError::Conflict);
        }
        if error.is_none() && current.snapshot.requires_execution {
            return Err(StoreError::MissingExecution);
        }
        let time = now_ms()?;
        let next = if error.is_some() {
            RunState::Failed
        } else {
            RunState::Succeeded
        };
        let label = super::runs::status(next);
        let answer = response["answer"].as_str().unwrap_or("");
        tx.execute("UPDATE execution_turns SET answer=$2,response_json=$3,status=$4,updated_at=$5 WHERE id=$1",&[&current.turn_id,&answer,&encoded(&response)?,&label,&time])?;
        tx.execute(
            "UPDATE execution_runs SET status=$2,error=$3,finished_at=$4 WHERE id=$1",
            &[&run, &label, &error, &time],
        )?;
        if let Some(usage) = response.get("usage") {
            tx.execute(
                "UPDATE execution_run_budgets SET usage_json=$2,updated_at=$3 WHERE run_id=$1",
                &[&run, &encoded(usage)?, &time],
            )?;
        }
        append(
            &mut tx,
            session,
            run,
            ExecutionEvent::RunTransition {
                run_id: run.into(),
                from: current.state,
                to: next,
                recovery: None,
            },
        )?;
        let payload = if let Some(message) = error {
            // A failed later step must not hide earlier committed deliverables.
            // The UI presents these as saved outputs, not successful completion.
            let mut payload = json!({"type":"failed","status":"failed","message":message,"public_notice":response.get("public_notice").cloned().unwrap_or(Value::Null),"artifacts":response.get("artifacts").cloned().unwrap_or_else(||json!([]))});
            if response["partial_result"] == true {
                payload["response"] = response;
            }
            payload
        } else {
            json!({"type":"complete","response":response})
        };
        append(&mut tx, session, run, ExecutionEvent::Chat { payload })?;
        tx.commit()?;
        Ok(())
    }
    pub fn chat_cancel(&self, owner: &str, run: &str) -> Result<RunRecord> {
        let session = self.owned_run(owner, run)?.session_id;
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_session(&mut tx, owner, &session)?;
        let current = read_run(&mut tx, &session, run)?;
        if current.state == RunState::Cancelled {
            return Ok(current);
        }
        if !current.state.permits(RunState::Cancelled) {
            return Err(StoreError::Conflict);
        }
        let time = now_ms()?;
        let mut control = super::control::load(&mut tx, run)?;
        control.recovery = None;
        super::control::save(&mut tx, run, &control)?;
        tx.execute("UPDATE execution_runs SET status='cancelled',cancel_requested=1,finished_at=$2 WHERE id=$1", &[&run, &time])?;
        tx.execute(
            "UPDATE execution_turns SET status='cancelled',updated_at=$2 WHERE id=$1",
            &[&current.turn_id, &time],
        )?;
        append(
            &mut tx,
            &session,
            run,
            ExecutionEvent::RunTransition {
                run_id: run.into(),
                from: current.state,
                to: RunState::Cancelled,
                recovery: None,
            },
        )?;
        append(
            &mut tx,
            &current.session_id,
            run,
            ExecutionEvent::Chat {
                payload: json!({"type":"cancelled","status":"cancelled","message":"运行已停止"}),
            },
        )?;
        let cancelled = read_run(&mut tx, &session, run)?;
        tx.commit()?;
        Ok(cancelled)
    }
    /// Host shutdown reconciles only workers it owned, never other processes
    /// or a tool workflow. No model/tool request is replayed by this operation.
    pub fn finish_stopped_chats(&self, runs: &[String]) -> Result<()> {
        for run in runs {
            let owner: Option<String> = self.connection()?.query_opt(
                "SELECT s.owner_user_id FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id WHERE r.id=$1 AND s.archived=0", &[&run])?.map(|row| row.get(0));
            let Some(owner) = owner else { continue };
            let current = self.owned_run(&owner, run)?;
            if current.state.active()
                && current.snapshot.model.settings["execution_kind"] == "text_chat"
                && !current.snapshot.requires_execution
                && current.snapshot.capabilities.is_empty()
            {
                match self.finish_chat(
                    &owner,
                    &current.session_id,
                    run,
                    json!({}),
                    Some("服务已停止，模型请求已终止；不会自动重复提交"),
                ) {
                    Ok(()) | Err(StoreError::Conflict) => {}
                    Err(error) => return Err(error),
                }
            }
        }
        Ok(())
    }
}
