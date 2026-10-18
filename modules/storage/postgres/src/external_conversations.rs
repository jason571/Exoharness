// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Administrator-only projections, limited to registered external Sessions.
use crate::{PostgresStore, Result, StoreError, codec};
use postgres::{Row, types::ToSql};
use serde_json::{Value, json};

pub struct ExternalConversationQuery {
    pub client_id: Option<String>,
    pub query: String,
    pub matching_users: Vec<String>,
    pub user_ids: Option<Vec<String>>,
    pub device: String,
    pub page: usize,
    pub limit: usize,
}

const SELECT: &str = "SELECT e.session_id,e.client_id,e.owner_user_id,e.assistant_id,e.device_id,s.title,s.created_at,s.updated_at,(SELECT count(*) FROM execution_turns t WHERE t.session_id=s.id),r.id,r.status FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id AND s.owner_user_id=e.owner_user_id LEFT JOIN LATERAL (SELECT id,status FROM execution_runs WHERE session_id=s.id ORDER BY created_at DESC,id DESC LIMIT 1) r ON true";
const FILTER: &str = "s.archived=0 AND ($1::text IS NULL OR e.client_id=$1) AND ($2='' OR strpos(lower(s.title),lower($2))>0 OR strpos(lower(e.session_id),lower($2))>0 OR strpos(lower(e.owner_user_id),lower($2))>0 OR e.owner_user_id=ANY($3::text[]) OR strpos(lower(COALESCE(e.device_id,'')),lower($2))>0 OR EXISTS (SELECT 1 FROM execution_turns t WHERE t.session_id=s.id AND strpos(lower(t.prompt || ' ' || COALESCE(t.answer,'')),lower($2))>0)) AND ($4::text[] IS NULL OR e.owner_user_id=ANY($4)) AND ($5='' OR strpos(lower(COALESCE(e.device_id,'')),lower($5))>0)";

fn summary(row: &Row) -> Value {
    json!({"id":row.get::<_,String>(0),"client_id":row.get::<_,String>(1),"user_id":row.get::<_,String>(2),"assistant_id":row.get::<_,String>(3),"device_id":row.get::<_,Option<String>>(4),"title":row.get::<_,String>(5),"created_at":row.get::<_,i64>(6),"updated_at":row.get::<_,i64>(7),"turn_count":row.get::<_,i64>(8),"latest_run_id":row.get::<_,Option<String>>(9),"latest_run_status":row.get::<_,Option<String>>(10)})
}

impl PostgresStore {
    pub fn management_external_conversations(
        &self,
        query: ExternalConversationQuery,
    ) -> Result<Value> {
        if !(1..=10).contains(&query.limit)
            || query.page == 0
            || query.page > 1_000_000
            || query.query.len() > 512
            || query.device.len() > 128
        {
            return Err(StoreError::Invalid(
                "invalid external conversation page or filter",
            ));
        }
        if let Some(client) = &query.client_id {
            codec::identifier(client)?;
        }
        let offset = ((query.page - 1) * query.limit) as i64;
        let limit = query.limit as i64;
        let mut connection = self.connection()?;
        let mut tx = connection
            .build_transaction()
            .isolation_level(postgres::IsolationLevel::RepeatableRead)
            .start()?;
        let filters: &[&(dyn ToSql + Sync)] = &[
            &query.client_id,
            &query.query,
            &query.matching_users,
            &query.user_ids,
            &query.device,
        ];
        let total: i64 = tx.query_one(&format!("SELECT count(*) FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id AND s.owner_user_id=e.owner_user_id WHERE {FILTER}"),filters)?.get(0);
        let mut params = filters.to_vec();
        params.extend([&limit as &(dyn ToSql + Sync), &offset]);
        let sessions: Vec<Value> = tx.query(&format!("{SELECT} WHERE {FILTER} ORDER BY s.updated_at DESC,e.session_id DESC LIMIT $6 OFFSET $7"),&params)?.iter().map(summary).collect();
        tx.commit()?;
        Ok(
            json!({"sessions":sessions,"total":total,"page":query.page,"page_size":query.limit,"has_more":offset+limit<total}),
        )
    }

    pub fn management_external_conversation(
        &self,
        client: &str,
        session: &str,
        before: Option<i64>,
        limit: usize,
    ) -> Result<Value> {
        if !(1..=10).contains(&limit) {
            return Err(StoreError::Invalid("history page size must be 1..10"));
        }
        let row = self
            .connection()?
            .query_opt(
                &format!("{SELECT} WHERE e.client_id=$1 AND e.session_id=$2 AND s.archived=0"),
                &[&client, &session],
            )?
            .ok_or(StoreError::NotFound)?;
        let external = summary(&row);
        let owner = row.get::<_, String>(2);
        let mut result = self.conversation_detail(&owner, session, before, limit)?;
        result["external"] = external;
        Ok(result)
    }
}
