// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Confidential application keys authenticate namespaced backend user assertions.
use super::{Application, ApplicationInput, Grant, IssuedTokens, grants::valid_grant, validation};
use crate::{
    AuthError, AuthStore,
    helpers::{now_seconds, token_digest, validate_display_name},
};
use exoharness_database::{OptionalExtension, params};
use sha2::{Digest, Sha256};

impl AuthStore {
    pub fn matching_external_user_ids(&self, query: &str) -> Result<Vec<String>, AuthError> {
        let connection = self.connection()?;
        let mut statement=connection.prepare("SELECT user_id FROM auth_external_user_links WHERE strpos(lower(external_subject),lower(?1::TEXT))>0")?;
        statement
            .query_map([query], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
    pub fn external_user_projection(
        &self,
        client: &str,
        user: &str,
    ) -> Result<Option<(String, String)>, AuthError> {
        self.connection()?.query_row("SELECT l.external_subject,u.display_name FROM auth_external_user_links l JOIN auth_users u ON u.id=l.user_id WHERE l.client_id=?1 AND l.user_id=?2",params![client,user],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(Into::into)
    }
    pub fn external_backend_tokens(
        &self,
        client: &str,
        key: &str,
        subject: &str,
        name: Option<&str>,
        device: Option<&str>,
    ) -> Result<Option<IssuedTokens>, AuthError> {
        let Some(grant) = self.external_backend_grant(client, key, subject, name, device)? else {
            return Ok(None);
        };
        let tx = self.connection()?.transaction()?;
        let record=tx.query_row("SELECT configuration_json,revision FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL FOR SHARE",[client],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).optional()?;
        let Some((raw, revision)) = record else {
            return Ok(None);
        };
        let Some((grant, _)) = valid_grant(&tx, &grant.id)? else {
            return Ok(None);
        };
        let app = Application {
            input: serde_json::from_str(&raw)
                .map_err(|_| validation::invalid("invalid stored application"))?,
            revision,
        };
        let result = super::tokens::issue(&tx, &grant, &app)?;
        tx.commit()?;
        Ok(Some(result))
    }

    pub fn external_backend_grant(
        &self,
        client: &str,
        key: &str,
        subject: &str,
        name: Option<&str>,
        device: Option<&str>,
    ) -> Result<Option<Grant>, AuthError> {
        if !validation::identifier(client) || !(32..=256).contains(&key.len()) {
            return Ok(None);
        }
        if subject.is_empty()
            || subject.len() > 128
            || subject.trim() != subject
            || subject.chars().any(char::is_control)
        {
            return Err(validation::invalid("invalid external user ID"));
        }
        if device.is_some_and(|id| {
            id.is_empty() || id.len() > 128 || id.trim() != id || id.chars().any(char::is_control)
        }) {
            return Err(validation::invalid("invalid device ID"));
        }
        let name = name.map(validate_display_name).transpose()?;
        let tx = self.connection()?.transaction()?;
        let record=tx.query_row("SELECT configuration_json,revision,secret_digest FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL FOR SHARE",[client],|r|Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?,r.get::<_,String>(2)?))).optional()?;
        let Some((raw, revision, digest)) = record else {
            return Ok(None);
        };
        if !validation::secret_matches(&token_digest(key), &digest) {
            return Ok(None);
        }
        let app: ApplicationInput = serde_json::from_str(&raw)
            .map_err(|_| validation::invalid("invalid stored application"))?;
        if !app.enabled || !app.allow_backend_identity {
            return Ok(None);
        }
        if app.require_device_id && device.is_none() {
            return Err(validation::invalid(
                "this application requires a verified device ID",
            ));
        }
        let lock = format!("external-user:{client}:{subject}");
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended(?1::TEXT,4271072004))",
            [lock],
        )?;
        let linked=tx.query_row("SELECT user_id FROM auth_external_user_links WHERE client_id=?1 AND external_subject=?2",params![client,subject],|r|r.get::<_,String>(0)).optional()?;
        let user = if let Some(user) = linked {
            user
        } else {
            let identity = serde_json::to_vec(&(client, subject))
                .map_err(|_| validation::invalid("invalid user identity"))?;
            let hash = format!("{:x}", Sha256::digest(identity));
            let user = format!("user_{}", validation::token("")?);
            let username = format!("app_{}", &hash[..28]);
            let normalized_email = format!("external:{hash}");
            let label = name.as_deref().unwrap_or(subject);
            tx.execute("INSERT INTO auth_users(id,username,normalized_username,email,normalized_email,display_name,password_hash,default_agent_profile_id,revision,created_at,updated_at) VALUES(?1,?2,?2,'',?3,?4,'!external-application',?5,1,?6,?6)",params![user,username,normalized_email,label,crate::DEFAULT_AGENT_PROFILE_ID,now_seconds()])?;
            tx.execute("INSERT INTO auth_external_user_links(client_id,external_subject,user_id,created_at) VALUES(?1,?2,?3,?4)",params![client,subject,user,now_seconds()])?;
            user
        };
        if let Some(name) = &name {
            tx.execute("UPDATE auth_users SET display_name=?2,updated_at=?3 WHERE id=?1 AND display_name<>?2",params![user,name,now_seconds()])?;
        }
        let auth_revision = tx.query_row(
            "SELECT revision FROM auth_users WHERE id=?1 FOR SHARE",
            [&user],
            |r| r.get::<_, i64>(0),
        )?;
        let previous=tx.query_row("SELECT id,revoked_at FROM auth_external_grants WHERE client_id=?1 AND user_id=?2 AND device_id IS NOT DISTINCT FROM ?3::TEXT AND source='backend' AND app_revision=?4 AND auth_revision=?5 AND expires_at>?6 ORDER BY created_at DESC,id DESC LIMIT 1",params![client,user,device,revision,auth_revision,now_seconds()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<i64>>(1)?))).optional()?;
        let grant = if let Some((id, revoked)) = previous {
            if revoked.is_some() {
                return Err(AuthError::Conflict(
                    "administrator revoked this application user/device grant".into(),
                ));
            }
            id
        } else {
            let id = validation::token("exg_")?;
            let scopes = serde_json::to_string(&app.scopes)
                .map_err(|_| validation::invalid("invalid scopes"))?;
            tx.execute("INSERT INTO auth_external_grants(id,client_id,user_id,app_revision,auth_revision,scopes_json,created_at,expires_at,device_id,source) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'backend')",params![id,client,user,revision,auth_revision,scopes,now_seconds(),now_seconds()+app.grant_ttl_seconds,device])?;
            id
        };
        let result = valid_grant(&tx, &grant)?.map(|(grant, _)| grant);
        tx.commit()?;
        Ok(result)
    }
}
