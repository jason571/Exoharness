use super::verify_scope;
use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};

impl PostgresStore {
    /// Compact manifest; full table data is not sent on list/search/detail requests.
    pub fn document_blocks(&self, scope: &str, id: &str) -> Result<Value> {
        let mut conn = self.connection()?;
        let mut tx = conn.transaction()?;
        verify_scope(&mut tx, scope)?;
        let row = tx.query_opt("SELECT revision,extracted_json,source_file_json FROM knowledge_documents WHERE owner_user_id=$1 AND id=$2", &[&scope,&id])?.ok_or(StoreError::NotFound)?;
        let parsed: Option<Value> = row.get(1);
        let source: Option<Value> = row.get(2);
        let blocks = parsed.as_ref().and_then(|p|p["blocks"].as_array()).map(|blocks| blocks.iter().map(|block| {
            json!({"id":block["id"],"kind":block["kind"],"source":block["source"],"row_count":block["row_count"],"column_count":block["column_count"]})
        }).collect::<Vec<_>>()).unwrap_or_default();
        Ok(
            json!({"document_id":id,"revision":row.get::<_,i64>(0),"blocks":blocks,"warnings":parsed.as_ref().map(|p|&p["warnings"]),"source_file":source.map(|s|json!({"filename":s["filename"],"sha256":s["sha256"],"size_bytes":s["size_bytes"]}))}),
        )
    }
    /// Version-pinned bounded view with header cells and overlapping merged anchors.
    pub fn read_document_block(
        &self,
        scope: &str,
        id: &str,
        block_id: &str,
        revision: i64,
        offset: usize,
        limit: usize,
    ) -> Result<Value> {
        if limit == 0 || limit > 100 || offset > 100_000 {
            return Err(StoreError::Invalid("invalid block range"));
        }
        let mut conn = self.connection()?;
        let row = conn.query_opt("SELECT revision,extracted_json FROM knowledge_documents WHERE owner_user_id=$1 AND id=$2", &[&scope,&id])?.ok_or(StoreError::NotFound)?;
        if row.get::<_, i64>(0) != revision {
            return Err(StoreError::Conflict);
        }
        let parsed: Option<Value> = row.get(1);
        let block = parsed
            .as_ref()
            .and_then(|p| p["blocks"].as_array())
            .and_then(|blocks| blocks.iter().find(|b| b["id"].as_str() == Some(block_id)))
            .ok_or(StoreError::NotFound)?;
        let mut result = block.clone();
        if let Some(rows) = block["rows"].as_array() {
            let end = offset.saturating_add(limit).min(rows.len());
            if offset > rows.len() {
                return Err(StoreError::Invalid("range outside table"));
            }
            let headers = block["header_rows"]
                .as_array()
                .map(|indexes| {
                    indexes
                        .iter()
                        .filter_map(|i| i.as_u64())
                        .filter_map(|i| rows.get(i as usize))
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let merged = rows
                .iter()
                .filter_map(Value::as_array)
                .flatten()
                .filter(|cell| {
                    let start = cell["row"].as_u64().unwrap_or(0) as usize;
                    let span = cell["rowspan"].as_u64().unwrap_or(1) as usize;
                    start < offset && start.saturating_add(span) > offset
                })
                .cloned()
                .collect::<Vec<_>>();
            result["rows"] = json!(&rows[offset..end]);
            result["headers"] = json!(headers);
            // Preserve the first row without guessing it is a semantic header.
            result["first_row_context"] = json!(rows.first());
            result["merged_anchors"] = json!(merged);
            result["offset"] = json!(offset);
            result["partial"] = json!(offset != 0 || end != rows.len());
        } else if let Some(pages) = block["pages"].as_array() {
            if offset > pages.len() {
                return Err(StoreError::Invalid("range outside pages"));
            }
            let end = offset.saturating_add(limit).min(pages.len());
            result["pages"] = json!(&pages[offset..end]);
            result["partial"] = json!(offset != 0 || end != pages.len());
        }
        Ok(json!({"document_id":id,"revision":revision,"block":result}))
    }
}
