// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Atomic application/session association and idempotent creation. Existing
//! browser sessions and ownership checks retain their original behavior.
use crate::{PostgresStore, Result, StoreError, codec};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) fn validate_device(device: &str) -> Result<()> {
    if device.is_empty()
        || device.len() > 128
        || device.trim() != device
        || device.chars().any(char::is_control)
    {
        return Err(StoreError::Invalid("invalid device ID"));
    }
    Ok(())
}

pub struct ExternalSessionRequest<'a> {
    pub owner: &'a str,
    pub client_id: &'a str,
    pub request_key: &'a str,
    pub title: Option<&'a str>,
    pub profile: &'a Value,
}
impl PostgresStore {
    /// Returns the existing receipt for the same input, including after a
    /// restart. Reusing a key for different input is a conflict.
    pub fn create_external_session(&self, request: ExternalSessionRequest<'_>) -> Result<Value> {
        self.create_external_session_with_device(request, None)
    }
    pub fn create_external_session_with_device(
        &self,
        request: ExternalSessionRequest<'_>,
        device_id: Option<&str>,
    ) -> Result<Value> {
        if let Some(device) = device_id {
            validate_device(device)?;
        }
        let ExternalSessionRequest {
            owner,
            client_id,
            request_key,
            title,
            profile,
        } = request;
        codec::identifier(owner)?;
        codec::identifier(client_id)?;
        if request_key.is_empty()
            || request_key.len() > 128
            || !request_key.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err(StoreError::Invalid("invalid idempotency key"));
        }
        let assistant = profile["id"]
            .as_str()
            .ok_or(StoreError::Invalid("missing assistant"))?;
        let label = title.unwrap_or("新对话");
        if label.is_empty() || label.len() > 512 || label.contains('\0') {
            return Err(StoreError::Invalid("invalid title"));
        }
        let key_digest = format!("{:x}", Sha256::digest(request_key.as_bytes()));
        let digest = format!(
            "{:x}",
            Sha256::digest(
                codec::encoded(&json!({"title":title,"assistant_id":assistant}))?.as_bytes()
            )
        );
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let lock = format!("external-session:{client_id}:{owner}:{key_digest}");
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,4271072003))",
            &[&lock],
        )?;
        if let Some(row)=tx.query_opt("SELECT e.session_id,e.request_digest,s.archived,e.device_id FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id WHERE e.client_id=$1 AND e.owner_user_id=$2 AND e.request_key_digest=$3 FOR UPDATE OF e",&[&client_id,&owner,&key_digest])? {
            if row.get::<_,String>(1)!=digest || row.get::<_,i64>(2)!=0 {return Err(StoreError::Conflict);}
            let id:String=row.get(0);
            if let Some(device) = device_id {
                if row.get::<_,Option<String>>(3).as_deref().is_some_and(|saved| saved != device) {
                    return Err(StoreError::Conflict);
                }
                tx.execute("UPDATE execution_external_sessions SET device_id=$2 WHERE session_id=$1 AND device_id IS NULL", &[&id,&device])?;
            }
            tx.commit()?;
            return self.conversation_detail(owner,&id,None,8);
        }
        let mut random = [0u8; 16];
        getrandom::fill(&mut random).map_err(|_| StoreError::Connection)?;
        let id = format!(
            "session-{}",
            random
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let source = if title.is_some() {
            "manual"
        } else {
            "placeholder"
        };
        tx.execute("INSERT INTO execution_sessions(id,title,created_at,updated_at,owner_user_id,title_source) VALUES($1,$2,$3,$3,$4,$5)",&[&id,&label,&codec::now_ms()?,&owner,&source])?;
        tx.execute("INSERT INTO execution_session_agent_profiles(session_id,profile_snapshot_json) VALUES($1,$2)",&[&id,&codec::encoded(profile)?])?;
        tx.execute("INSERT INTO execution_external_sessions(session_id,client_id,owner_user_id,assistant_id,request_key_digest,request_digest,device_id) VALUES($1,$2,$3,$4,$5,$6,$7)",&[&id,&client_id,&owner,&assistant,&key_digest,&digest,&device_id])?;
        tx.commit()?;
        self.conversation_detail(owner, &id, None, 8)
    }

    /// Add metadata once; it cannot move a conversation to a different device.
    pub fn set_external_session_device(
        &self,
        owner: &str,
        client: &str,
        assistant: &str,
        session: &str,
        device: &str,
    ) -> Result<Value> {
        validate_device(device)?;
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        let row = tx.query_opt("SELECT e.device_id FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id AND s.owner_user_id=e.owner_user_id WHERE e.session_id=$1 AND e.owner_user_id=$2 AND e.client_id=$3 AND e.assistant_id=$4 AND s.archived=0 FOR UPDATE OF e", &[&session,&owner,&client,&assistant])?.ok_or(StoreError::NotFound)?;
        if row
            .get::<_, Option<String>>(0)
            .as_deref()
            .is_some_and(|saved| saved != device)
        {
            return Err(StoreError::Conflict);
        }
        tx.execute(
            "UPDATE execution_external_sessions SET device_id=$2 WHERE session_id=$1",
            &[&session, &device],
        )?;
        tx.commit()?;
        Ok(json!({"session_id":session,"device_id":device}))
    }
    pub fn external_session_authorized(
        &self,
        owner: &str,
        client: &str,
        assistant: &str,
        session: &str,
    ) -> Result<bool> {
        Ok(self.connection()?.query_opt("SELECT e.session_id FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id AND s.owner_user_id=e.owner_user_id WHERE e.session_id=$1 AND e.owner_user_id=$2 AND e.client_id=$3 AND e.assistant_id=$4 AND s.archived=0",&[&session,&owner,&client,&assistant])?.is_some())
    }

    pub fn external_session_device(
        &self,
        owner: &str,
        client: &str,
        session: &str,
    ) -> Result<Option<String>> {
        let row = self.connection()?.query_opt("SELECT e.device_id FROM execution_external_sessions e JOIN execution_sessions s ON s.id=e.session_id AND s.owner_user_id=e.owner_user_id WHERE e.session_id=$1 AND e.owner_user_id=$2 AND e.client_id=$3 AND s.archived=0", &[&session,&owner,&client])?.ok_or(StoreError::NotFound)?;
        Ok(row.get(0))
    }
}
