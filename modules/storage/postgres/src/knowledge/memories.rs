// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{expected, text, timestamp};
use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};

impl PostgresStore {
    pub fn memories(&self, owner: &str) -> Result<Vec<Value>> {
        self.connection()?.query("SELECT id,content,enabled,source,revision FROM personal_memories WHERE owner_user_id=$1 ORDER BY updated_at DESC,id LIMIT 500", &[&owner])?
            .iter().map(|row| Ok(json!({"id":row.get::<_,String>(0),"content":row.get::<_,String>(1),"enabled":row.get::<_,bool>(2),"source":row.get::<_,String>(3),"revision":row.get::<_,i64>(4)}))).collect()
    }
    pub fn save_memory(&self, owner: &str, id: &str, create: bool, body: &Value) -> Result<Value> {
        let content = text(body, "content", 16 * 1024)?;
        let enabled = body["enabled"]
            .as_bool()
            .ok_or(StoreError::Invalid("enabled required"))?;
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let revision = if create {
            tx.execute("INSERT INTO personal_memories(id,owner_user_id,content,enabled,source,revision,created_at,updated_at) VALUES($1,$2,$3,$4,'user',1,$5,$5)", &[&id,&owner,&content,&enabled,&timestamp()])?;
            1
        } else {
            let row = tx.query_opt("SELECT revision FROM personal_memories WHERE owner_user_id=$1 AND id=$2 FOR UPDATE", &[&owner,&id])?.ok_or(StoreError::NotFound)?;
            let revision: i64 = row.get(0);
            expected(body, revision)?;
            tx.execute("UPDATE personal_memories SET content=$3,enabled=$4,revision=revision+1,updated_at=$5 WHERE owner_user_id=$1 AND id=$2", &[&owner,&id,&content,&enabled,&timestamp()])?;
            revision + 1
        };
        tx.commit()?;
        Ok(json!({"id":id,"content":content,"enabled":enabled,"source":"user","revision":revision}))
    }
    pub fn delete_memory(&self, owner: &str, id: &str) -> Result<()> {
        if self.connection()?.execute(
            "DELETE FROM personal_memories WHERE owner_user_id=$1 AND id=$2",
            &[&owner, &id],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}
