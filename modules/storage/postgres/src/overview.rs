//! Read-only overview of committed journal facts. No execution is submitted.
use crate::{PostgresStore, Result, StoreError, codec::now_ms};
use postgres::IsolationLevel;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

impl PostgresStore {
    /// Aggregation precedes display limits. Status uses the latest attempt;
    /// usage includes all attempts, matching the preserved management page.
    pub fn management_overview(&self, days: u64, user: Option<&str>) -> Result<Value> {
        if !(1..=365).contains(&days) || user.is_some_and(|value| value.len() > 512) {
            return Err(StoreError::Invalid(
                "invalid overview period or user filter",
            ));
        }
        let generated = now_ms()?;
        let since = generated.saturating_sub((days * 86_400_000) as i64);
        let user = user.filter(|value| !value.is_empty());
        let mut connection = self.connection()?;
        let mut tx = connection
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()?;
        // Constants only; values always use bound parameters. position() means
        // '%' and '_' in a username are literal, not wildcard expressions.
        let scope = "($2::TEXT IS NULL OR position(lower($2::TEXT) in lower(COALESCE(s.owner_user_id,'management')))>0)";
        let latest = "NOT EXISTS (SELECT 1 FROM execution_runs newer WHERE newer.turn_id=r.turn_id AND newer.attempt>r.attempt)";
        let counts: BTreeMap<String, i64> = tx.query(
            &format!("SELECT r.status,COUNT(*) FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id
                WHERE (r.created_at>=$1 OR r.status NOT IN ('succeeded','failed','cancelled','recoverable'))
                AND {scope} AND {latest} GROUP BY r.status"), &[&since, &user],
        )?.into_iter().map(|row| (row.get(0), row.get(1))).collect();
        let mut groups = Map::new();
        for (name, expression) in [
            ("users", "COALESCE(s.owner_user_id,'management')"),
            (
                "assistants",
                "COALESCE(p.profile_snapshot_json::jsonb->>'id','unassigned')",
            ),
        ] {
            let rows = tx.query(&format!("SELECT {expression},COUNT(*),
                COUNT(*) FILTER(WHERE r.status='succeeded'),COUNT(*) FILTER(WHERE r.status IN ('failed','recoverable')),
                COALESCE(SUM((b.usage_json::jsonb->>'total_tokens')::BIGINT),0)::BIGINT,
                COALESCE(SUM(CASE WHEN r.started_at IS NOT NULL AND r.finished_at IS NOT NULL THEN GREATEST(r.finished_at-r.started_at,0) ELSE 0 END),0)::BIGINT
                FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id
                LEFT JOIN execution_run_budgets b ON b.run_id=r.id
                LEFT JOIN execution_run_agent_profiles p ON p.run_id=r.id
                WHERE r.created_at>=$1 AND {scope} GROUP BY {expression} ORDER BY 5 DESC,1 LIMIT 50"), &[&since,&user])?;
            groups.insert(
                name.into(),
                json!(
                    rows.into_iter()
                        .map(|row| json!({
                            "id":row.get::<_,String>(0),"runs":row.get::<_,i64>(1),
                            "succeeded":row.get::<_,i64>(2),"attention":row.get::<_,i64>(3),
                            "tokens":row.get::<_,i64>(4),"completed_duration_ms":row.get::<_,i64>(5)
                        }))
                        .collect::<Vec<_>>()
                ),
            );
        }
        let models = tx
            .query(
                &format!(
                    "SELECT m.provider,m.model,COUNT(*),
            COALESCE(SUM((m.usage_json::jsonb->>'total_tokens')::BIGINT),0)::BIGINT,
            COUNT(*) FILTER(WHERE m.usage_reported=0),COALESCE(SUM(m.duration_ms),0)::BIGINT
            FROM execution_model_calls m JOIN execution_runs r ON r.id=m.run_id
            JOIN execution_sessions s ON s.id=r.session_id WHERE m.started_at>=$1 AND {scope}
            GROUP BY m.provider,m.model ORDER BY 4 DESC,1,2 LIMIT 50"
                ),
                &[&since, &user],
            )?
            .into_iter()
            .map(|row| {
                json!({"provider":row.get::<_,String>(0),"model":row.get::<_,String>(1),
                "requests":row.get::<_,i64>(2),"tokens":row.get::<_,i64>(3),
                "unreported_requests":row.get::<_,i64>(4),"duration_ms":row.get::<_,i64>(5)})
            })
            .collect::<Vec<_>>();
        let totals = tx.query_one(&format!("SELECT COUNT(*),COALESCE(SUM((b.usage_json::jsonb->>'total_tokens')::BIGINT),0)::BIGINT,
            COUNT(*) FILTER(WHERE r.status='succeeded') FROM execution_runs r
            JOIN execution_sessions s ON s.id=r.session_id LEFT JOIN execution_run_budgets b ON b.run_id=r.id
            WHERE r.created_at>=$1 AND {scope}"), &[&since,&user])?;
        let activity = tx.query(&format!("SELECT r.id,r.session_id,left(COALESCE(NULLIF(t.prompt,''),s.title),160),r.status,r.created_at,r.started_at,
            COALESCE(s.owner_user_id,'management'),t.ordinal,r.attempt,r.turn_id
            FROM execution_runs r JOIN execution_sessions s ON s.id=r.session_id JOIN execution_turns t ON t.id=r.turn_id
            WHERE (r.created_at>=$1 OR r.status NOT IN ('succeeded','failed','cancelled','recoverable')) AND {scope} AND {latest}
            ORDER BY CASE WHEN r.status NOT IN ('succeeded','failed','cancelled','recoverable') THEN 0 ELSE 1 END,r.created_at DESC,r.id DESC LIMIT 50"), &[&since,&user])?
            .into_iter().map(|row| json!({"id":row.get::<_,String>(0),"session_id":row.get::<_,String>(1),
                "title":row.get::<_,String>(2),"status":row.get::<_,String>(3),"created_at":row.get::<_,i64>(4),
                "started_at":row.get::<_,Option<i64>>(5),"user_id":row.get::<_,String>(6),
                "turn_ordinal":row.get::<_,i64>(7),"attempt":row.get::<_,i64>(8),"turn_id":row.get::<_,String>(9)})).collect::<Vec<_>>();
        let value = json!({"generated_at":generated,"days":days,"since":since,"counts":counts,
            "runs":totals.get::<_,i64>(0),"tokens":totals.get::<_,i64>(1),"succeeded":totals.get::<_,i64>(2),
            "groups":groups,"models":models,"activity":activity,"group_limit":50});
        tx.commit()?;
        Ok(value)
    }
}
