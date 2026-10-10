// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{SUMMARY, owned, summary};
use crate::{PostgresStore, Result, StoreError, codec};
use serde_json::{Value, json};

impl PostgresStore {
    pub fn conversations(&self, owner: &str, query: &str) -> Result<Vec<Value>> {
        if query.len() > 512 {
            return Err(StoreError::Invalid("search too long"));
        }
        self.connection()?.query(&format!("{SUMMARY} WHERE s.owner_user_id=$1 AND s.archived=0 AND ($2='' OR strpos(lower(s.title),lower($2))>0) ORDER BY s.pinned DESC,s.updated_at DESC,s.id LIMIT 200"), &[&owner,&query])?.iter().map(summary).collect()
    }
    pub fn new_conversation(
        &self,
        owner: &str,
        id: &str,
        title: &str,
        profile: &Value,
    ) -> Result<Value> {
        self.new_conversation_with_title(owner, id, title, profile, false)
    }
    pub fn new_conversation_with_title(
        &self,
        owner: &str,
        id: &str,
        title: &str,
        profile: &Value,
        automatic_title: bool,
    ) -> Result<Value> {
        codec::identifier(id)?;
        if title.is_empty() || title.len() > 512 || title.contains('\0') {
            return Err(StoreError::Invalid("invalid title"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let source = if automatic_title {
            "placeholder"
        } else {
            "manual"
        };
        tx.execute("INSERT INTO execution_sessions(id,title,created_at,updated_at,owner_user_id,title_source) VALUES($1,$2,$3,$3,$4,$5)", &[&id,&title,&codec::now_ms()?,&owner,&source])?;
        tx.execute("INSERT INTO execution_session_agent_profiles(session_id,profile_snapshot_json) VALUES($1,$2)", &[&id,&codec::encoded(profile)?])?;
        let result = summary(&tx.query_one(&format!("{SUMMARY} WHERE s.id=$1"), &[&id])?)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn update_conversation(&self, owner: &str, id: &str, body: &Value) -> Result<Value> {
        let fields = body
            .as_object()
            .ok_or(StoreError::Invalid("object required"))?;
        if fields.is_empty()
            || fields
                .keys()
                .any(|key| !matches!(key.as_str(), "title" | "pinned"))
        {
            return Err(StoreError::Invalid("unknown conversation field"));
        }
        let title = body
            .get("title")
            .map(|v| {
                v.as_str()
                    .filter(|s| !s.trim().is_empty() && s.len() <= 512 && !s.contains('\0'))
                    .ok_or(StoreError::Invalid("invalid title"))
            })
            .transpose()?;
        let pinned = body
            .get("pinned")
            .map(|v| {
                v.as_bool()
                    .map(i64::from)
                    .ok_or(StoreError::Invalid("invalid pinned flag"))
            })
            .transpose()?;
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        owned(&mut tx, owner, id)?;
        tx.execute("UPDATE execution_sessions SET title=COALESCE($3,title),pinned=COALESCE($4,pinned),updated_at=$5 WHERE owner_user_id=$1 AND id=$2", &[&owner,&id,&title,&pinned,&codec::now_ms()?])?;
        if title.is_some() {
            tx.execute("UPDATE execution_sessions SET title_source='manual' WHERE owner_user_id=$1 AND id=$2", &[&owner,&id])?;
        }
        let result = summary(&tx.query_one(&format!("{SUMMARY} WHERE s.id=$1"), &[&id])?)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn delete_conversation(&self, owner: &str, id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        owned(&mut tx, owner, id)?;
        if tx.query_opt("SELECT id FROM execution_runs WHERE session_id=$1 AND status IN ('queued','running','waiting_authorization') LIMIT 1", &[&id])?.is_some() {return Err(StoreError::Conflict);}
        // Archive rather than destroy the execution evidence or generated files.
        tx.execute("UPDATE execution_sessions SET archived=1,updated_at=$3 WHERE owner_user_id=$1 AND id=$2", &[&owner,&id,&codec::now_ms()?])?;
        tx.commit()?;
        Ok(())
    }
    pub fn conversation_detail(
        &self,
        owner: &str,
        id: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Value> {
        if !(1..=100).contains(&limit) || before.is_some_and(|n| n <= 0) {
            return Err(StoreError::Invalid("invalid history page"));
        }
        let mut conn = self.connection()?;
        let mut tx = conn
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::RepeatableRead)
            .start()?;
        let row = tx
            .query_opt(
                &format!("{SUMMARY} WHERE s.owner_user_id=$1 AND s.id=$2 AND s.archived=0"),
                &[&owner, &id],
            )?
            .ok_or(StoreError::NotFound)?;
        let mut result = summary(&row)?;
        let rows=tx.query("SELECT t.id,t.ordinal,t.prompt,t.attachments_json,t.status,t.answer,t.response_json,t.created_at,t.updated_at,r.id,r.status,r.error,g.source_run_id FROM execution_turns t LEFT JOIN LATERAL (SELECT id,status,error FROM execution_runs WHERE turn_id=t.id ORDER BY attempt DESC LIMIT 1) r ON true LEFT JOIN execution_regenerations g ON g.turn_id=t.id WHERE t.session_id=$1 AND ($2::bigint IS NULL OR t.ordinal<$2) ORDER BY t.ordinal DESC LIMIT $3", &[&id,&before,&(limit as i64+1)])?;
        let has_more = rows.len() > limit;
        let turns=rows.iter().take(limit).rev().map(|row|{
            let response:Option<String>=row.get(6);
            Ok(json!({"id":row.get::<_,String>(0),"ordinal":row.get::<_,i64>(1),"prompt":row.get::<_,String>(2),"attachments":codec::parsed::<Value>(&row.get::<_,String>(3))?,"status":row.get::<_,String>(4),"answer":row.get::<_,Option<String>>(5),"response":response.map(|v|codec::parsed::<Value>(&v)).transpose()?,"created_at":row.get::<_,i64>(7),"updated_at":row.get::<_,i64>(8),"run_id":row.get::<_,Option<String>>(9),"run_status":row.get::<_,Option<String>>(10),"run_error":row.get::<_,Option<String>>(11),"regenerated_from_run_id":row.get::<_,Option<String>>(12)}))
        }).collect::<Result<Vec<_>>>()?;
        let notices:Vec<Value>=tx.query("SELECT id,kind,content,sender,created_at FROM execution_session_notices WHERE session_id=$1 ORDER BY created_at DESC,id LIMIT 100", &[&id])?.iter().map(|row|json!({"id":row.get::<_,String>(0),"kind":row.get::<_,String>(1),"content":row.get::<_,String>(2),"sender":row.get::<_,Option<String>>(3),"created_at":row.get::<_,i64>(4)})).collect();
        result["history_page"] = json!({"has_more":has_more,"before_ordinal":turns.first().map(|v|v["ordinal"].clone()),"limit":limit});
        result["turns"] = json!(turns);
        result["notices"] = json!(notices);
        Ok(result)
    }
}
