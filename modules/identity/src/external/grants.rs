// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{Application, AuthorizationRequest, Grant, validation};
use crate::{
    AuthError, AuthStore,
    helpers::{now_seconds, token_digest},
};
use exoharness_database::{Executor, OptionalExtension, params};

pub(super) fn valid_grant<C: Executor>(
    connection: &C,
    id: &str,
) -> Result<Option<(Grant, i64)>, AuthError> {
    let record = connection.query_row(
        "SELECT g.client_id,g.user_id,g.scopes_json,g.expires_at,a.configuration_json,a.revision,g.device_id FROM auth_external_grants g JOIN auth_users u ON u.id=g.user_id AND u.revision=g.auth_revision JOIN auth_external_applications a ON a.client_id=g.client_id AND a.revision=g.app_revision WHERE g.id=?1 AND a.deleted_at IS NULL AND g.revoked_at IS NULL AND g.expires_at>?2",
        params![id, now_seconds()],
        |r| Ok((r.get::<_,String>(0)?, r.get::<_,String>(1)?, r.get::<_,String>(2)?, r.get::<_,i64>(3)?, r.get::<_,String>(4)?, r.get::<_,i64>(5)?, r.get::<_,Option<String>>(6)?)),
    ).optional()?;
    let Some((client_id, user_id, raw_scopes, expires_at, raw_app, revision, device_id)) = record
    else {
        return Ok(None);
    };
    let app: super::ApplicationInput = serde_json::from_str(&raw_app)
        .map_err(|_| validation::invalid("invalid stored application"))?;
    if !app.enabled {
        return Ok(None);
    }
    let scopes = serde_json::from_str(&raw_scopes)
        .map_err(|_| validation::invalid("invalid stored grant"))?;
    Ok(Some((
        Grant {
            id: id.into(),
            client_id,
            user_id,
            scopes,
            expires_at,
            assistant_id: app.assistant_id,
            require_device_id: app.require_device_id,
            device_id,
        },
        revision,
    )))
}
impl AuthStore {
    pub fn external_application_grants(
        &self,
        client: &str,
    ) -> Result<Vec<serde_json::Value>, AuthError> {
        let connection = self.connection()?;
        let mut statement=connection.prepare("SELECT g.id,g.user_id,u.username,u.display_name,g.scopes_json,g.created_at,g.expires_at,g.revoked_at,(g.revoked_at IS NULL AND g.expires_at>?2 AND u.revision=g.auth_revision AND a.revision=g.app_revision AND (a.configuration_json::jsonb->>'enabled')::boolean) FROM auth_external_grants g JOIN auth_users u ON u.id=g.user_id JOIN auth_external_applications a ON a.client_id=g.client_id WHERE g.client_id=?1 ORDER BY g.created_at DESC,g.id LIMIT 200")?;
        statement.query_map(params![client,now_seconds()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?,r.get::<_,i64>(5)?,r.get::<_,i64>(6)?,r.get::<_,Option<i64>>(7)?,r.get::<_,bool>(8)?)))?
            .map(|row|{let(id,user,username,name,scopes,created,expires,revoked,active)=row?;let scopes:Vec<String>=serde_json::from_str(&scopes).map_err(|_|validation::invalid("invalid stored scopes"))?;Ok(serde_json::json!({"id":id,"client_id":client,"user_id":user,"username":username,"display_name":name,"scopes":scopes,"created_at":created,"expires_at":expires,"revoked_at":revoked,"active":active}))}).collect()
    }
    pub fn revoke_external_application_grant(
        &self,
        client: &str,
        id: &str,
    ) -> Result<bool, AuthError> {
        Ok(self.connection()?.execute("UPDATE auth_external_grants SET revoked_at=COALESCE(revoked_at,?3) WHERE client_id=?1 AND id=?2",params![client,id,now_seconds()])?==1)
    }
    pub fn external_grant(&self, id: &str) -> Result<Option<Grant>, AuthError> {
        valid_grant(&self.connection()?, id).map(|value| value.map(|(grant, _)| grant))
    }
    pub fn validate_external_authorization(
        &self,
        request: &AuthorizationRequest,
    ) -> Result<Application, AuthError> {
        let app = self
            .external_application(&request.client_id)?
            .ok_or_else(|| validation::invalid("unknown application"))?;
        validation::authorization(request, &app.input)?;
        Ok(app)
    }
    /// Account ownership comes exclusively from the authenticated browser.
    pub fn authorize_external_application(
        &self,
        user: &str,
        request: &AuthorizationRequest,
    ) -> Result<String, AuthError> {
        self.authorize_external(user, None, request)?
            .ok_or_else(|| validation::invalid("invalid user authorization"))
    }
    /// Recheck the actual login session under row locks when minting a grant.
    /// A request authenticated before logout/password rotation must not bind
    /// its new grant to the account's replacement authentication revision.
    pub fn authorize_external_application_from_session(
        &self,
        user: &str,
        session: &str,
        request: &AuthorizationRequest,
    ) -> Result<Option<String>, AuthError> {
        self.authorize_external(user, Some(session), request)
    }
    fn authorize_external(
        &self,
        user: &str,
        session: Option<&str>,
        request: &AuthorizationRequest,
    ) -> Result<Option<String>, AuthError> {
        let tx = self.connection()?.transaction()?;
        let record = tx.query_row("SELECT configuration_json,revision FROM auth_external_applications WHERE client_id=?1 AND deleted_at IS NULL FOR SHARE", [&request.client_id], |r| Ok((r.get::<_,String>(0)?,r.get::<_,i64>(1)?))).optional()?;
        let Some((raw, revision)) = record else {
            return Ok(None);
        };
        let app: super::ApplicationInput = serde_json::from_str(&raw)
            .map_err(|_| validation::invalid("invalid stored application"))?;
        let scopes = validation::authorization(request, &app)?;
        let user_revision: i64 = if let Some(session) = session {
            let revision = tx.query_row(
                "SELECT u.revision FROM auth_users u JOIN auth_user_sessions s ON s.user_id=u.id AND s.auth_revision=u.revision WHERE u.id=?1 AND s.token_digest=?2 AND s.expires_at>?3 FOR SHARE OF u,s",
                params![user, token_digest(session), now_seconds()], |r| r.get(0),
            ).optional()?;
            let Some(revision) = revision else {
                return Ok(None);
            };
            revision
        } else {
            tx.query_row(
                "SELECT revision FROM auth_users WHERE id=?1 FOR SHARE",
                [user],
                |r| r.get(0),
            )?
        };
        let now = now_seconds();
        let grant = validation::token("exg_")?;
        let code = validation::token("exc_")?;
        let raw_scopes =
            serde_json::to_string(&scopes).map_err(|_| validation::invalid("invalid scopes"))?;
        tx.execute("INSERT INTO auth_external_grants(id,client_id,user_id,app_revision,auth_revision,scopes_json,created_at,expires_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)", params![grant,app.client_id,user,revision,user_revision,raw_scopes,now,now + app.grant_ttl_seconds])?;
        tx.execute("INSERT INTO auth_external_codes(code_digest,grant_id,redirect_uri,code_challenge,expires_at) VALUES(?1,?2,?3,?4,?5)", params![token_digest(&code),grant,request.redirect_uri,request.code_challenge,now+120])?;
        tx.commit()?;
        let mut redirect = validation::redirect(&request.redirect_uri)?;
        redirect
            .query_pairs_mut()
            .append_pair("code", &code)
            .append_pair("state", &request.state);
        Ok(Some(redirect.into()))
    }
    pub fn external_grants(&self, user: &str) -> Result<Vec<Grant>, AuthError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id FROM auth_external_grants WHERE user_id=?1 AND revoked_at IS NULL AND expires_at>?2 ORDER BY created_at DESC")?;
        let ids = statement
            .query_map(params![user, now_seconds()], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.iter()
            .filter_map(|id| match valid_grant(&connection, id) {
                Ok(Some((grant, _))) => Some(Ok(grant)),
                Ok(None) => None,
                Err(e) => Some(Err(e)),
            })
            .collect()
    }
    pub fn revoke_external_grant(&self, user: &str, id: &str) -> Result<bool, AuthError> {
        Ok(self.connection()?.execute("UPDATE auth_external_grants SET revoked_at=COALESCE(revoked_at,?3) WHERE user_id=?1 AND id=?2",params![user,id,now_seconds()])? == 1)
    }
}
