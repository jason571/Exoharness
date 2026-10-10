//! Management-only, read-only projections of the existing execution journal.
mod detail;
mod evidence;
mod lists;
use crate::{Result, StoreError};
use postgres::Row;
use serde_json::{Value, json};

fn row_json(row: Row) -> Result<Value> {
    Ok(serde_json::from_str(&row.get::<_, String>(0))?)
}
fn decode_field(value: &mut Value, source: &str, target: &str) -> Result<()> {
    let raw = value
        .as_object_mut()
        .ok_or(StoreError::Decode)?
        .remove(source);
    value[target] = match raw {
        Some(Value::String(text)) => serde_json::from_str(&text)?,
        _ => Value::Null,
    };
    Ok(())
}
fn usage(value: &mut Value) {
    for (target, source) in [
        ("input", "input_tokens"),
        ("output", "output_tokens"),
        ("cache_read", "cache_read_tokens"),
        ("cache_write", "cache_write_tokens"),
    ] {
        value[target] = value[source].clone();
    }
    value["reported"] = json!({"input":value["input"].is_number(),"output":value["output"].is_number(),"cache_read":value["cache_read"].is_number(),"cache_write":value["cache_write"].is_number()});
}
fn page(limit: usize, offset: usize) -> Result<(i64, i64)> {
    if !(1..=200).contains(&limit) || offset > 1_000_000 {
        return Err(StoreError::Invalid("invalid trace page"));
    }
    Ok((limit as i64, offset as i64))
}
