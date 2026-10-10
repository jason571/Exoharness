use super::{page, row_json};
use crate::{PostgresStore, Result};
use serde_json::Value;
impl PostgresStore {
    pub fn trace_sessions(
        &self,
        session: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Value>> {
        let (limit, offset) = page(limit, offset)?;
        self.connection()?.query("SELECT (to_jsonb(s) || jsonb_build_object(
            'status',COALESCE((SELECT status FROM execution_runs WHERE session_id=s.id ORDER BY created_at DESC,id DESC LIMIT 1),'pending'),
            'latest_prompt',COALESCE((SELECT prompt FROM execution_turns WHERE session_id=s.id ORDER BY ordinal DESC LIMIT 1),''),
            'turn_count',(SELECT COUNT(*) FROM execution_turns WHERE session_id=s.id),
            'run_count',(SELECT COUNT(*) FROM execution_runs WHERE session_id=s.id),
            'model_call_count',(SELECT COUNT(*) FROM execution_model_calls m JOIN execution_runs r ON r.id=m.run_id WHERE r.session_id=s.id),
            'invocation_count',(SELECT COUNT(*) FROM execution_invocations i JOIN execution_turns t ON t.id=i.turn_id WHERE t.session_id=s.id),
            'total_tokens',COALESCE((SELECT SUM((m.usage_json::jsonb->>'total_tokens')::bigint) FROM execution_model_calls m JOIN execution_runs r ON r.id=m.run_id WHERE r.session_id=s.id),0)
        ))::text FROM execution_sessions s WHERE EXISTS(SELECT 1 FROM execution_runs r WHERE r.session_id=s.id) AND ($1::text IS NULL OR s.id=$1) ORDER BY s.updated_at DESC,s.id DESC LIMIT $2 OFFSET $3",&[&session,&limit,&offset])?.into_iter().map(row_json).collect()
    }
    pub fn trace_runs(&self, session: &str, limit: usize, offset: usize) -> Result<Vec<Value>> {
        let (limit, offset) = page(limit, offset)?;
        self.connection()?.query("SELECT (to_jsonb(r)-'model_snapshot_json'-'capability_snapshot_json' || jsonb_build_object(
            'turn_ordinal',t.ordinal,'prompt_preview',left(t.prompt,300),
            'total_tokens',COALESCE((SELECT SUM((usage_json::jsonb->>'total_tokens')::bigint) FROM execution_model_calls WHERE run_id=r.id),0)
        ))::text FROM execution_runs r JOIN execution_turns t ON t.id=r.turn_id JOIN execution_sessions s ON s.id=r.session_id WHERE r.session_id=$1 ORDER BY r.created_at DESC,r.id DESC LIMIT $2 OFFSET $3",&[&session,&limit,&offset])?.into_iter().map(row_json).collect()
    }
}
