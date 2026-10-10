use super::owned;
use crate::{PostgresStore, Result, StoreError, codec};
use postgres::Transaction;
use serde_json::{Value, json};

fn run_owned(tx: &mut Transaction<'_>, owner: &str, run: &str) -> Result<String> {
    let row=tx.query_opt("SELECT r.session_id FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id WHERE r.id=$1 AND s.owner_user_id=$2 AND s.archived=0", &[&run,&owner])?.ok_or(StoreError::NotFound)?;
    let session: String = row.get(0);
    owned(tx, owner, &session)?;
    Ok(session)
}
fn state(tx: &mut Transaction<'_>, actor: &str, run: &str) -> Result<Value> {
    let row=tx.query_opt("SELECT action,reason,detail FROM execution_response_actions WHERE run_id=$1 AND actor=$2 AND action IN ('like','dislike','clear_feedback') ORDER BY sequence DESC LIMIT 1", &[&run,&actor])?;
    Ok(match row {
        Some(row) => {
            let action: String = row.get(0);
            json!({"feedback":if action=="clear_feedback" {None}else{Some(action)},"reason":row.get::<_,Option<String>>(1),"detail":row.get::<_,String>(2),"requires_tool_confirmation":true})
        }
        None => {
            json!({"feedback":null,"reason":null,"detail":"","requires_tool_confirmation":true})
        }
    })
}
impl PostgresStore {
    pub fn response_feedback(&self, owner: &str, run: &str) -> Result<Value> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        run_owned(&mut tx, owner, run)?;
        state(&mut tx, owner, run)
    }
    pub fn conversation_feedback(
        &self,
        owner: &str,
        session: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Value> {
        if !(1..=100).contains(&limit) {
            return Err(StoreError::Invalid("invalid history page"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        owned(&mut tx, owner, session)?;
        let rows=tx.query("SELECT r.id FROM execution_turns t JOIN LATERAL (SELECT id FROM execution_runs WHERE turn_id=t.id ORDER BY attempt DESC LIMIT 1) r ON true WHERE t.session_id=$1 AND ($2::bigint IS NULL OR t.ordinal<$2) ORDER BY t.ordinal DESC LIMIT $3", &[&session,&before,&(limit as i64)])?;
        let mut result = serde_json::Map::new();
        for row in rows {
            let run: String = row.get(0);
            result.insert(run.clone(), state(&mut tx, owner, &run)?);
        }
        Ok(Value::Object(result))
    }
    pub fn record_response_feedback(&self, owner: &str, run: &str, body: &Value) -> Result<Value> {
        let event = body["event_id"]
            .as_str()
            .ok_or(StoreError::Invalid("event id required"))?;
        codec::identifier(event)?;
        let action = body["action"]
            .as_str()
            .ok_or(StoreError::Invalid("action required"))?;
        if !matches!(action, "copy" | "like" | "dislike" | "clear_feedback") {
            return Err(StoreError::Invalid("unsupported response action"));
        }
        let detail = match body.get("detail") {
            None => "",
            Some(v) => v.as_str().ok_or(StoreError::Invalid("invalid detail"))?,
        };
        if detail.chars().count() > 200 || detail.contains('\0') {
            return Err(StoreError::Invalid("feedback detail too long"));
        }
        let reason = match body.get("reason") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .filter(|s| s.len() <= 128 && !s.contains('\0'))
                    .ok_or(StoreError::Invalid("invalid feedback reason"))?,
            ),
        };
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        // Serializes an actor's event ids even when submitted against different Runs.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072003))",
            &[&owner],
        )?;
        run_owned(&mut tx, owner, run)?;
        if let Some(row)=tx.query_opt("SELECT run_id,action,reason,detail FROM execution_response_actions WHERE actor=$1 AND event_id=$2", &[&owner,&event])? {
            if row.get::<_,String>(0)!=run || row.get::<_,String>(1)!=action || row.get::<_,Option<String>>(2).as_deref()!=reason || row.get::<_,String>(3)!=detail {return Err(StoreError::Conflict);}
        } else {
            tx.execute("INSERT INTO execution_response_actions(event_id,run_id,actor,action,reason,detail,created_at) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&event,&run,&owner,&action,&reason,&detail,&codec::now_ms()?])?;
        }
        let result = state(&mut tx, owner, run)?;
        tx.commit()?;
        Ok(result)
    }
}
