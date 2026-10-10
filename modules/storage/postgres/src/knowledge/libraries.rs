// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{expected, lock_scope, optional, text, timestamp};
use crate::{PostgresStore, Result, StoreError};
use postgres::Transaction;
use serde_json::{Value, json};

fn project(tx: &mut Transaction<'_>, row: &postgres::Row) -> Result<Value> {
    let id: String = row.get(0);
    let assistants:Vec<String>=tx.query("SELECT assistant_id FROM knowledge_library_grants WHERE library_id=$1 ORDER BY assistant_id", &[&id])?.iter().map(|row|row.get(0)).collect();
    Ok(
        json!({"id":id,"name":row.get::<_,String>(1),"description":row.get::<_,String>(2),"revision":row.get::<_,i64>(3),"assistant_ids":assistants}),
    )
}
impl PostgresStore {
    pub fn knowledge_libraries(&self, id: Option<&str>) -> Result<Vec<Value>> {
        let mut conn = self.connection()?;
        let mut tx = conn
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::RepeatableRead)
            .start()?;
        let rows=tx.query("SELECT id,name,description,revision FROM knowledge_libraries WHERE ($1::text IS NULL OR id=$1) ORDER BY name,id", &[&id])?;
        if id.is_some() && rows.is_empty() {
            return Err(StoreError::NotFound);
        }
        rows.iter().map(|row| project(&mut tx, row)).collect()
    }
    pub fn save_knowledge_library(&self, id: &str, create: bool, body: &Value) -> Result<Value> {
        let name = text(body, "name", 256)?;
        let description = optional(body, "description")?.unwrap_or_default();
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        if create {
            tx.execute("INSERT INTO knowledge_libraries(id,name,description,revision,created_at,updated_at) VALUES($1,$2,$3,1,$4,$4)", &[&id,&name,&description,&timestamp()])?;
        } else {
            lock_scope(&mut tx, &format!("library:{id}"))?;
            let row = tx.query_one(
                "SELECT revision FROM knowledge_libraries WHERE id=$1",
                &[&id],
            )?;
            expected(body, row.get(0))?;
            tx.execute("UPDATE knowledge_libraries SET name=$2,description=$3,revision=revision+1,updated_at=$4 WHERE id=$1", &[&id,&name,&description,&timestamp()])?;
        }
        let row = tx.query_one(
            "SELECT id,name,description,revision FROM knowledge_libraries WHERE id=$1",
            &[&id],
        )?;
        let result = project(&mut tx, &row)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn delete_knowledge_library(&self, id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        let scope = format!("library:{id}");
        lock_scope(&mut tx, &scope)?;
        tx.execute(
            "DELETE FROM knowledge_documents WHERE owner_user_id=$1",
            &[&scope],
        )?;
        tx.execute(
            "DELETE FROM knowledge_categories WHERE scope_owner=$1",
            &[&scope],
        )?;
        tx.execute("DELETE FROM knowledge_libraries WHERE id=$1", &[&id])?;
        tx.commit()?;
        Ok(())
    }
    pub fn grant_knowledge_library(&self, id: &str, assistants: &[String]) -> Result<Value> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_scope(&mut tx, &format!("library:{id}"))?;
        tx.execute(
            "DELETE FROM knowledge_library_grants WHERE library_id=$1",
            &[&id],
        )?;
        for assistant in assistants {
            tx.execute("INSERT INTO knowledge_library_grants(library_id,assistant_id,created_at) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&id,&assistant,&timestamp()])?;
        }
        tx.execute(
            "UPDATE knowledge_libraries SET revision=revision+1,updated_at=$2 WHERE id=$1",
            &[&id, &timestamp()],
        )?;
        let row = tx.query_one(
            "SELECT id,name,description,revision FROM knowledge_libraries WHERE id=$1",
            &[&id],
        )?;
        let result = project(&mut tx, &row)?;
        tx.commit()?;
        Ok(result)
    }
}
