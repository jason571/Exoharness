// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{Application, Grant, IssuedTokens, grants::valid_grant, validation};
use crate::{
    AuthError, AuthStore,
    helpers::{now_seconds, token_digest},
};
use exoharness_database::{OptionalExtension, Transaction, params};

pub(super) fn issue(
    tx: &Transaction,
    grant: &Grant,
    app: &Application,
) -> Result<IssuedTokens, AuthError> {
    let access = validation::token("exa_")?;
    let refresh = validation::token("exr_")?;
    let ttl = app
        .input
        .access_ttl_seconds
        .min(grant.expires_at - now_seconds());
    for (token, kind, expires) in [
        (&access, "access", now_seconds() + ttl),
        (&refresh, "refresh", grant.expires_at),
    ] {
        tx.execute("INSERT INTO auth_external_tokens(token_digest,grant_id,kind,expires_at) VALUES(?1,?2,?3,?4)",params![token_digest(token),grant.id,kind,expires])?;
    }
    Ok(IssuedTokens {
        access_token: access,
        token_type: "Bearer",
        expires_in: ttl,
        refresh_token: refresh,
        scope: grant.scopes.join(" "),
        user_id: grant.user_id.clone(),
        grant_id: grant.id.clone(),
    })
}
impl AuthStore {
    pub fn exchange_external_code(
        &self,
        app: &Application,
        code: &str,
        redirect: &str,
        verifier: &str,
    ) -> Result<Option<IssuedTokens>, AuthError> {
        if !code.starts_with("exc_") || code.len() > 256 {
            return Ok(None);
        }
        let challenge = super::pkce_challenge(verifier)?;
        let tx = self.connection()?.transaction()?;
        let record = tx.query_row("SELECT grant_id,redirect_uri,code_challenge,consumed_at,expires_at FROM auth_external_codes WHERE code_digest=?1 FOR UPDATE",[token_digest(code)],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,Option<i64>>(3)?,r.get::<_,i64>(4)?))).optional()?;
        let Some((id, uri, expected, used, expires)) = record else {
            return Ok(None);
        };
        let Some((grant, revision)) = valid_grant(&tx, &id)? else {
            return Ok(None);
        };
        if grant.client_id != app.input.client_id
            || revision != app.revision
            || uri != redirect
            || !validation::secret_matches(&challenge, &expected)
            || expires <= now_seconds()
        {
            return Ok(None);
        }
        if used.is_some() {
            tx.execute(
                "UPDATE auth_external_grants SET revoked_at=?2 WHERE id=?1",
                params![id, now_seconds()],
            )?;
            tx.commit()?;
            return Ok(None);
        }
        tx.execute(
            "UPDATE auth_external_codes SET consumed_at=?2 WHERE code_digest=?1",
            params![token_digest(code), now_seconds()],
        )?;
        let result = issue(&tx, &grant, app)?;
        tx.commit()?;
        Ok(Some(result))
    }
    pub fn refresh_external_token(
        &self,
        app: &Application,
        token: &str,
    ) -> Result<Option<IssuedTokens>, AuthError> {
        if !token.starts_with("exr_") || token.len() > 256 {
            return Ok(None);
        };
        let tx = self.connection()?.transaction()?;
        let record = tx.query_row("SELECT grant_id,consumed_at,expires_at FROM auth_external_tokens WHERE token_digest=?1 AND kind='refresh' FOR UPDATE",[token_digest(token)],|r|Ok((r.get::<_,String>(0)?,r.get::<_,Option<i64>>(1)?,r.get::<_,i64>(2)?))).optional()?;
        let Some((id, used, expires)) = record else {
            return Ok(None);
        };
        let Some((grant, revision)) = valid_grant(&tx, &id)? else {
            return Ok(None);
        };
        if grant.client_id != app.input.client_id
            || revision != app.revision
            || expires <= now_seconds()
        {
            return Ok(None);
        };
        if used.is_some() {
            tx.execute(
                "UPDATE auth_external_grants SET revoked_at=?2 WHERE id=?1",
                params![id, now_seconds()],
            )?;
            tx.commit()?;
            return Ok(None);
        }
        tx.execute(
            "UPDATE auth_external_tokens SET consumed_at=?2 WHERE token_digest=?1",
            params![token_digest(token), now_seconds()],
        )?;
        let result = issue(&tx, &grant, app)?;
        tx.commit()?;
        Ok(Some(result))
    }
    pub fn validate_external_access(&self, token: &str) -> Result<Option<Grant>, AuthError> {
        if !token.starts_with("exa_") || token.len() > 256 {
            return Ok(None);
        };
        let connection = self.connection()?;
        let id=connection.query_row("SELECT grant_id FROM auth_external_tokens WHERE token_digest=?1 AND kind='access' AND consumed_at IS NULL AND expires_at>?2",params![token_digest(token),now_seconds()],|r|r.get::<_,String>(0)).optional()?;
        id.map(|id| valid_grant(&connection, &id).map(|v| v.map(|(grant, _)| grant)))
            .transpose()
            .map(Option::flatten)
    }
    pub fn revoke_external_token(&self, app: &Application, token: &str) -> Result<(), AuthError> {
        if token.len() > 256 {
            return Ok(());
        };
        self.connection()?.execute("UPDATE auth_external_grants SET revoked_at=COALESCE(revoked_at,?3) WHERE client_id=?1 AND id IN (SELECT grant_id FROM auth_external_tokens WHERE token_digest=?2)",params![app.input.client_id,token_digest(token),now_seconds()])?;
        Ok(())
    }
}
