// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};
use std::collections::BTreeMap;
impl PostgresStore {
    pub fn user_execution_summaries(&self, owners: &[String]) -> Result<BTreeMap<String, Value>> {
        if owners.len() > 100 {
            return Err(StoreError::Invalid("user page too large"));
        }
        let rows=self.connection()?.query("WITH runs AS (SELECT r.session_id,COUNT(*) AS run_count,COALESCE(SUM((b.usage_json::jsonb->>'total_tokens')::BIGINT),0)::BIGINT AS tokens FROM execution_runs r LEFT JOIN execution_run_budgets b ON b.run_id=r.id GROUP BY r.session_id) SELECT s.owner_user_id,COUNT(*),COALESCE(SUM(r.run_count),0)::BIGINT,COALESCE(SUM(r.tokens),0)::BIGINT,MAX(s.updated_at) FROM execution_sessions s LEFT JOIN runs r ON r.session_id=s.id WHERE s.archived=0 AND s.owner_user_id=ANY($1) GROUP BY s.owner_user_id", &[&owners])?;
        Ok(rows.into_iter().map(|row|(row.get(0),json!({"conversation_count":row.get::<_,i64>(1),"run_count":row.get::<_,i64>(2),"total_tokens":row.get::<_,i64>(3),"last_activity_at":row.get::<_,Option<i64>>(4)}))).collect())
    }
    pub fn user_conversations(&self, owner: &str, limit: usize) -> Result<Vec<Value>> {
        if !(1..=100).contains(&limit) {
            return Err(StoreError::Invalid("conversation page size must be 1..100"));
        }
        let rows=self.connection()?.query("SELECT s.id,s.title,s.created_at,s.updated_at,(SELECT COUNT(*) FROM execution_turns t WHERE t.session_id=s.id),(SELECT COUNT(*) FROM execution_runs r WHERE r.session_id=s.id),(SELECT status FROM execution_runs r WHERE r.session_id=s.id ORDER BY r.created_at DESC,r.id DESC LIMIT 1),(SELECT prompt FROM execution_turns t WHERE t.session_id=s.id ORDER BY ordinal DESC LIMIT 1),COALESCE((SELECT SUM((b.usage_json::jsonb->>'total_tokens')::BIGINT)::BIGINT FROM execution_runs r JOIN execution_run_budgets b ON b.run_id=r.id WHERE r.session_id=s.id),0)::BIGINT FROM execution_sessions s WHERE s.owner_user_id=$1 AND s.archived=0 ORDER BY s.updated_at DESC,s.id LIMIT $2",&[&owner,&(limit as i64)])?;
        Ok(rows.into_iter().map(|row|json!({"id":row.get::<_,String>(0),"title":row.get::<_,String>(1),"created_at":row.get::<_,i64>(2),"updated_at":row.get::<_,i64>(3),"turn_count":row.get::<_,i64>(4),"run_count":row.get::<_,i64>(5),"latest_run_status":row.get::<_,Option<String>>(6),"latest_prompt":row.get::<_,Option<String>>(7),"total_tokens":row.get::<_,i64>(8)})).collect())
    }
}
