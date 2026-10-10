use super::{expected, lock_scope, optional, text, timestamp, verify_scope};
use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};

fn project(row: &postgres::Row) -> Value {
    json!({"id":row.get::<_,String>(0),"name":row.get::<_,String>(1),"parent_id":row.get::<_,Option<String>>(2),"path":row.get::<_,String>(3),"revision":row.get::<_,i64>(4)})
}
impl PostgresStore {
    pub fn knowledge_categories(&self, scope: &str) -> Result<Vec<Value>> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        verify_scope(&mut tx, scope)?;
        Ok(tx.query("SELECT id,name,parent_id,path,revision FROM knowledge_categories WHERE scope_owner=$1 ORDER BY path,id", &[&scope])?.iter().map(project).collect())
    }
    pub fn save_knowledge_category(
        &self,
        scope: &str,
        id: &str,
        create: bool,
        body: &Value,
    ) -> Result<Value> {
        let name = text(body, "name", 128)?;
        if name.contains('/') || name.chars().any(char::is_control) {
            return Err(StoreError::Invalid("invalid category name"));
        }
        let parent = optional(body, "parent_id")?;
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_scope(&mut tx, scope)?;
        let old = if create {
            None
        } else {
            let row=tx.query_opt("SELECT path,revision FROM knowledge_categories WHERE scope_owner=$1 AND id=$2 FOR UPDATE", &[&scope,&id])?.ok_or(StoreError::NotFound)?;
            expected(body, row.get(1))?;
            Some(row.get::<_, String>(0))
        };
        let path = if let Some(parent) = &parent {
            if parent == id {
                return Err(StoreError::Invalid("category cycle"));
            }
            let row = tx
                .query_opt(
                    "SELECT path FROM knowledge_categories WHERE scope_owner=$1 AND id=$2",
                    &[&scope, &parent],
                )?
                .ok_or(StoreError::NotFound)?;
            let parent_path: String = row.get(0);
            if old.as_ref().is_some_and(|old| {
                parent_path == *old || parent_path.starts_with(&format!("{old}/"))
            }) {
                return Err(StoreError::Invalid("category cycle"));
            }
            format!("{parent_path}/{name}")
        } else {
            name.to_owned()
        };
        if path.split('/').count() > 3 {
            return Err(StoreError::Invalid("category maximum depth is three"));
        }
        // Compute all affected paths first, then validate before changing any row.
        let mut changes = Vec::new();
        if let Some(old) = &old {
            for row in tx.query(
                "SELECT id,path FROM knowledge_categories WHERE scope_owner=$1 ORDER BY path",
                &[&scope],
            )? {
                let old_path: String = row.get(1);
                if let Some(suffix) = old_path.strip_prefix(&format!("{old}/")) {
                    let new_path = format!("{path}/{suffix}");
                    if new_path.split('/').count() > 3 {
                        return Err(StoreError::Invalid("subtree maximum depth is three"));
                    }
                    changes.push((row.get::<_, String>(0), new_path));
                }
            }
        }
        let mut destinations = vec![path.clone()];
        destinations.extend(changes.iter().map(|(_, path)| path.clone()));
        for row in tx.query(
            "SELECT id,path FROM knowledge_categories WHERE scope_owner=$1",
            &[&scope],
        )? {
            let other: String = row.get(0);
            if other != id
                && !changes.iter().any(|(changed, _)| changed == &other)
                && destinations.contains(&row.get::<_, String>(1))
            {
                return Err(StoreError::Conflict);
            }
        }
        if create {
            tx.execute("INSERT INTO knowledge_categories(id,scope_owner,name,parent_id,path,revision,created_at,updated_at) VALUES($1,$2,$3,$4,$5,1,$6,$6)", &[&id,&scope,&name,&parent,&path,&timestamp()])?;
        } else {
            tx.execute("UPDATE knowledge_categories SET name=$3,parent_id=$4,path=$5,revision=revision+1,updated_at=$6 WHERE scope_owner=$1 AND id=$2", &[&scope,&id,&name,&parent,&path,&timestamp()])?;
            for (changed, path) in changes {
                tx.execute("UPDATE knowledge_categories SET path=$3,revision=revision+1,updated_at=$4 WHERE scope_owner=$1 AND id=$2", &[&scope,&changed,&path,&timestamp()])?;
            }
        }
        let result=project(&tx.query_one("SELECT id,name,parent_id,path,revision FROM knowledge_categories WHERE scope_owner=$1 AND id=$2", &[&scope,&id])?);
        tx.commit()?;
        Ok(result)
    }
    pub fn delete_knowledge_category(&self, scope: &str, id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_scope(&mut tx, scope)?;
        if tx
            .query_opt(
                "SELECT id FROM knowledge_categories WHERE scope_owner=$1 AND id=$2",
                &[&scope, &id],
            )?
            .is_none()
        {
            return Err(StoreError::NotFound);
        }
        if tx
            .query_opt(
                "SELECT id FROM knowledge_categories WHERE scope_owner=$1 AND parent_id=$2 LIMIT 1",
                &[&scope, &id],
            )?
            .is_some()
        {
            return Err(StoreError::Conflict);
        }
        tx.execute("UPDATE knowledge_documents SET category_id=NULL,revision=revision+1,updated_at=$3 WHERE owner_user_id=$1 AND category_id=$2", &[&scope,&id,&timestamp()])?;
        tx.execute(
            "DELETE FROM knowledge_categories WHERE scope_owner=$1 AND id=$2",
            &[&scope, &id],
        )?;
        tx.commit()?;
        Ok(())
    }
}
