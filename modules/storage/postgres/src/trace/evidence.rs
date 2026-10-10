use crate::{PostgresStore, Result, StoreError};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

fn parts(request: &Value) -> Result<Vec<(Value, Value)>> {
    let mut config = request.clone();
    let fields = config.as_object_mut().ok_or(StoreError::Decode)?;
    for field in ["messages", "tools", "system"] {
        fields.remove(field);
    }
    let mut parts = Vec::new();
    for (kind, values) in [
        ("configuration", vec![config]),
        (
            "system",
            request.get("system").cloned().into_iter().collect(),
        ),
        (
            "message",
            request["messages"].as_array().cloned().unwrap_or_default(),
        ),
        (
            "tool",
            request["tools"].as_array().cloned().unwrap_or_default(),
        ),
    ] {
        for (index, value) in values.into_iter().enumerate() {
            let bytes = serde_json::to_vec(&value)?;
            parts.push((json!({"kind":kind,"index":index,"sha256":format!("{:x}",Sha256::digest(&bytes)),"bytes":bytes.len(),"redacted":false}),value));
        }
    }
    Ok(parts)
}
impl PostgresStore {
    pub fn trace_request(
        &self,
        run: &str,
        logical: &str,
        attempt: i64,
        part: Option<&str>,
    ) -> Result<Value> {
        let mut conn = self.connection()?;
        let row=conn.query_opt("SELECT q.manifest_json,q.sequence FROM execution_model_requests q JOIN execution_runs r ON r.id=q.run_id JOIN execution_sessions s ON s.id=r.session_id WHERE q.run_id=$1 AND q.logical_call_id=$2 AND q.attempt=$3",&[&run,&logical,&attempt])?.ok_or(StoreError::NotFound)?;
        let request: Value = serde_json::from_str(&row.get::<_, String>(0))?;
        let current = parts(&request)?;
        if let Some(digest) = part {
            return current
                .into_iter()
                .find(|(p, _)| p["sha256"] == digest)
                .map(|(p, v)| json!({"sha256":p["sha256"],"kind":p["kind"],"content":v}))
                .ok_or(StoreError::NotFound);
        }
        let sequence: i64 = row.get(1);
        let previous=conn.query_opt("SELECT manifest_json FROM execution_model_requests WHERE run_id=$1 AND sequence<$2 ORDER BY sequence DESC LIMIT 1",&[&run,&sequence])?;
        let previous = previous
            .map(|r| serde_json::from_str::<Value>(&r.get::<_, String>(0)))
            .transpose()?;
        let old = match &previous {
            Some(v) => parts(v)?,
            None => vec![],
        };
        let old_hashes: BTreeSet<_> = old
            .iter()
            .map(|(p, _)| p["sha256"].as_str().unwrap().to_owned())
            .collect();
        let current_hashes: BTreeSet<_> = current
            .iter()
            .map(|(p, _)| p["sha256"].as_str().unwrap().to_owned())
            .collect();
        let added: Vec<_> = current
            .iter()
            .filter(|(p, _)| !old_hashes.contains(p["sha256"].as_str().unwrap()))
            .map(|(p, _)| p.clone())
            .collect();
        let retained: Vec<_> = current
            .iter()
            .filter(|(p, _)| old_hashes.contains(p["sha256"].as_str().unwrap()))
            .map(|(p, _)| p.clone())
            .collect();
        let removed: Vec<_> = old
            .iter()
            .filter(|(p, _)| !current_hashes.contains(p["sha256"].as_str().unwrap()))
            .map(|(p, _)| p.clone())
            .collect();
        let manifest: Vec<_> = current.into_iter().map(|(p, _)| p).collect();
        Ok(
            json!({"manifest":{"parts":manifest},"changes":{"added":added,"removed":removed,"retained":retained},"exact_projection":true,"comparison_available":previous.is_some(),"transformations":[]}),
        )
    }
}
