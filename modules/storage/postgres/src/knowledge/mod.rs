//! Scoped knowledge persistence. No import execution or implicit network fetch.
mod blocks;
mod categories;
mod documents;
mod libraries;
mod memories;

use crate::{Result, StoreError};
use postgres::Transaction;
use serde_json::Value;

fn timestamp() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn text<'a>(body: &'a Value, field: &str, max: usize) -> Result<&'a str> {
    let value = body[field]
        .as_str()
        .ok_or(StoreError::Invalid("text field required"))?
        .trim();
    if value.is_empty() || value.len() > max || value.contains('\0') {
        return Err(StoreError::Invalid("text field length invalid"));
    }
    Ok(value)
}
fn expected(body: &Value, revision: i64) -> Result<()> {
    if body["expected_revision"].as_i64() != Some(revision) {
        return Err(StoreError::Conflict);
    }
    Ok(())
}
fn optional(body: &Value, field: &str) -> Result<Option<String>> {
    match body.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.len() <= 2048 && !value.contains('\0') => {
            Ok(Some(value.clone()))
        }
        _ => Err(StoreError::Invalid("invalid optional text")),
    }
}
// Every scope writer uses one lock, preventing hierarchy races and library-delete races.
fn lock_scope(tx: &mut Transaction<'_>, scope: &str) -> Result<()> {
    if scope.is_empty() || scope.len() > 512 {
        return Err(StoreError::Invalid("invalid knowledge scope"));
    }
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072002))",
        &[&scope],
    )?;
    if let Some(id) = scope.strip_prefix("library:")
        && tx
            .query_opt(
                "SELECT id FROM knowledge_libraries WHERE id=$1 FOR UPDATE",
                &[&id],
            )?
            .is_none()
    {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
fn verify_scope(tx: &mut Transaction<'_>, scope: &str) -> Result<()> {
    if let Some(id) = scope.strip_prefix("library:")
        && tx
            .query_opt("SELECT id FROM knowledge_libraries WHERE id=$1", &[&id])?
            .is_none()
    {
        return Err(StoreError::NotFound);
    }
    Ok(())
}
