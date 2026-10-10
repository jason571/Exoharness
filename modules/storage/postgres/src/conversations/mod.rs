//! Browser projections over the same durable session/turn/run records.
mod feedback;
mod history;
pub(crate) mod titles;
use crate::{Result, StoreError};
use postgres::Transaction;
use serde_json::{Value, json};

fn summary(row: &postgres::Row) -> Result<Value> {
    let profile: Option<String> = row.get(14);
    let profile = profile
        .map(|value| serde_json::from_str::<Value>(&value))
        .transpose()?
        .map(|profile| {
            if profile.is_null() {
                return Value::Null;
            }
            // Legacy snapshots may contain private prompt/access data. The browser
            // receives the same explicit public projection as the assistant header.
            let mut public = serde_json::Map::new();
            for key in [
                "id",
                "revision",
                "display_name",
                "avatar_text",
                "description",
                "digest",
                "software_engineering",
            ] {
                if let Some(value) = profile.get(key) {
                    public.insert(key.into(), value.clone());
                }
            }
            if let Some(avatar) = profile.get("avatar") {
                public.insert(
                    "avatar".into(),
                    if avatar.is_null() {
                        Value::Null
                    } else {
                        json!({"asset_id":avatar["asset_id"],"content_type":avatar["content_type"]})
                    },
                );
            }
            Value::Object(public)
        });
    Ok(
        json!({"id":row.get::<_,String>(0),"title":row.get::<_,String>(1),"pinned":row.get::<_,i64>(2)!=0,"created_at":row.get::<_,i64>(3),"updated_at":row.get::<_,i64>(4),"turn_count":row.get::<_,i64>(5),"latest_run_id":row.get::<_,Option<String>>(6),"latest_run_status":row.get::<_,Option<String>>(7),"managed_coding":row.get::<_,Option<i64>>(8).unwrap_or(0)!=0,"parent_id":row.get::<_,Option<String>>(9),"tagline":row.get::<_,Option<String>>(10),"cwd":row.get::<_,Option<String>>(11),"project_name":row.get::<_,Option<String>>(12),"version":row.get::<_,i64>(13),"agent_profile":profile}),
    )
}
const SUMMARY: &str = "SELECT s.id,s.title,s.pinned,s.created_at,s.updated_at,(SELECT COUNT(*) FROM execution_turns t WHERE t.session_id=s.id),r.id,r.status,m.managed_coding,m.parent_id,m.tagline,m.cwd,m.project_name,COALESCE((SELECT SUM(last_sequence) FROM execution_runs WHERE session_id=s.id),0)::bigint,p.profile_snapshot_json FROM execution_sessions s LEFT JOIN LATERAL (SELECT id,status FROM execution_runs WHERE session_id=s.id ORDER BY created_at DESC,id DESC LIMIT 1) r ON true LEFT JOIN execution_session_metadata m ON m.session_id=s.id LEFT JOIN execution_session_agent_profiles p ON p.session_id=s.id";
fn owned(tx: &mut Transaction<'_>, owner: &str, id: &str) -> Result<()> {
    if tx.query_opt("SELECT id FROM execution_sessions WHERE owner_user_id=$1 AND id=$2 AND archived=0 FOR UPDATE", &[&owner,&id])?.is_none() {return Err(StoreError::NotFound);}
    Ok(())
}
