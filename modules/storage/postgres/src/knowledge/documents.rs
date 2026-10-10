use super::{expected, lock_scope, optional, text, timestamp, verify_scope};
use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn project(row: &postgres::Row, full: bool) -> Result<Value> {
    let content: String = row.get(2);
    let tags: Value = serde_json::from_str(&row.get::<_, String>(8))?;
    let mut value = json!({"id":row.get::<_,String>(0),"title":row.get::<_,String>(1),"excerpt":content.chars().take(300).collect::<String>(),"sha256":row.get::<_,String>(3),"source_uri":row.get::<_,Option<String>>(4),"revision":row.get::<_,i64>(5),"category_id":row.get::<_,Option<String>>(6),"category_path":row.get::<_,Option<String>>(7),"tags":tags});
    if full {
        value["content"] = Value::String(content);
    }
    Ok(value)
}
impl PostgresStore {
    pub fn knowledge_documents(
        &self,
        scope: &str,
        id: Option<&str>,
        query: Option<&str>,
    ) -> Result<Vec<Value>> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        verify_scope(&mut tx, scope)?;
        let search = query.unwrap_or("").trim();
        if search.len() > 512 {
            return Err(StoreError::Invalid("search too long"));
        }
        // Literal search (not user-authored LIKE/SQL). Scope is applied before matching.
        let rows = tx.query("SELECT d.id,d.title,d.content,d.sha256,d.source_uri,d.revision,d.category_id,c.path,d.tags_json FROM knowledge_documents d LEFT JOIN knowledge_categories c ON c.id=d.category_id AND c.scope_owner=d.owner_user_id WHERE d.owner_user_id=$1 AND ($2::text IS NULL OR d.id=$2) AND ($3='' OR strpos(lower(d.title || ' ' || d.content),lower($3))>0) ORDER BY d.updated_at DESC,d.id LIMIT 200", &[&scope,&id,&search])?;
        if id.is_some() && rows.is_empty() {
            return Err(StoreError::NotFound);
        }
        rows.iter().map(|row| project(row, id.is_some())).collect()
    }
    pub fn save_knowledge_document(
        &self,
        scope: &str,
        id: &str,
        create: bool,
        body: &Value,
    ) -> Result<Value> {
        self.save_document(scope, id, create, body, None)
    }
    pub fn save_imported_document(
        &self,
        scope: &str,
        id: &str,
        body: &Value,
        parsed: &Value,
        source: &Value,
    ) -> Result<Value> {
        if parsed["blocks"].as_array().is_none() || source["sha256"].as_str().is_none() {
            return Err(StoreError::Invalid("invalid parsed document"));
        }
        self.save_document(scope, id, true, body, Some((parsed, source)))
    }
    fn save_document(
        &self,
        scope: &str,
        id: &str,
        create: bool,
        body: &Value,
        imported: Option<(&Value, &Value)>,
    ) -> Result<Value> {
        let title = text(body, "title", 512)?;
        let content = text(body, "content", 1024 * 1024)?;
        let uri = optional(body, "source_uri")?;
        let category = optional(body, "category_id")?;
        let tags = body
            .get("tags")
            .and_then(Value::as_array)
            .ok_or(StoreError::Invalid("tags array required"))?;
        if tags.len() > 32
            || tags
                .iter()
                .any(|v| v.as_str().is_none_or(|s| s.len() > 128 || s.contains('\0')))
        {
            return Err(StoreError::Invalid("invalid tags"));
        }
        let tags = serde_json::to_string(tags)?;
        let digest = format!("{:x}", Sha256::digest(content.as_bytes()));
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_scope(&mut tx, scope)?;
        if let Some(category) = &category
            && tx
                .query_opt(
                    "SELECT id FROM knowledge_categories WHERE scope_owner=$1 AND id=$2",
                    &[&scope, &category],
                )?
                .is_none()
        {
            return Err(StoreError::NotFound);
        }
        if create {
            tx.execute("INSERT INTO knowledge_documents(id,owner_user_id,title,content,sha256,source_kind,source_uri,revision,created_at,updated_at,category_id,tags_json) VALUES($1,$2,$3,$4,$5,'user',$6,1,$7,$7,$8,$9)", &[&id,&scope,&title,&content,&digest,&uri,&timestamp(),&category,&tags])?;
        } else {
            let row = tx.query_opt("SELECT revision FROM knowledge_documents WHERE owner_user_id=$1 AND id=$2 FOR UPDATE", &[&scope,&id])?.ok_or(StoreError::NotFound)?;
            expected(body, row.get(0))?;
            // Manual text edits invalidate the parsed structure, not the original evidence.
            tx.execute("UPDATE knowledge_documents SET extracted_json=NULL WHERE owner_user_id=$1 AND id=$2 AND content<>$3", &[&scope,&id,&content])?;
            tx.execute("UPDATE knowledge_documents SET title=$3,content=$4,sha256=$5,source_uri=$6,revision=revision+1,updated_at=$7,category_id=$8,tags_json=$9 WHERE owner_user_id=$1 AND id=$2", &[&scope,&id,&title,&content,&digest,&uri,&timestamp(),&category,&tags])?;
        }
        if let Some((parsed, source)) = imported {
            tx.execute("UPDATE knowledge_documents SET extracted_json=$3,source_file_json=$4,source_kind='file' WHERE owner_user_id=$1 AND id=$2", &[&scope,&id,&parsed,&source])?;
        }
        // Project inside the same transaction: do not return a racing later revision.
        let row = tx.query_one("SELECT d.id,d.title,d.content,d.sha256,d.source_uri,d.revision,d.category_id,c.path,d.tags_json FROM knowledge_documents d LEFT JOIN knowledge_categories c ON c.id=d.category_id AND c.scope_owner=d.owner_user_id WHERE d.owner_user_id=$1 AND d.id=$2", &[&scope,&id])?;
        let result = project(&row, true)?;
        tx.commit()?;
        Ok(result)
    }
    pub fn delete_knowledge_document(&self, scope: &str, id: &str) -> Result<()> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        lock_scope(&mut tx, scope)?;
        if tx.execute(
            "DELETE FROM knowledge_documents WHERE owner_user_id=$1 AND id=$2",
            &[&scope, &id],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        tx.commit()?;
        Ok(())
    }
}
