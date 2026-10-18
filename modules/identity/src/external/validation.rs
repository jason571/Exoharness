// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::{ApplicationInput, AuthorizationRequest, SCOPES};
use crate::{AuthError, helpers::random_session_token};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use url::Url;

pub(super) fn invalid(message: &str) -> AuthError {
    AuthError::Validation(message.into())
}
pub(super) fn identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
}
pub(super) fn token(prefix: &str) -> Result<String, AuthError> {
    Ok(format!("{prefix}{}", random_session_token()?.0))
}
pub(super) fn secret_matches(actual: &str, expected: &str) -> bool {
    actual.len() == expected.len()
        && actual
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |a, (b, c)| a | (b ^ c))
            == 0
}
pub(super) fn scopes(values: &[String]) -> Result<Vec<String>, AuthError> {
    if values.is_empty()
        || values.len() > SCOPES.len()
        || values.iter().any(|v| !SCOPES.contains(&v.as_str()))
    {
        return Err(invalid("invalid application scopes"));
    }
    Ok(values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}
pub(super) fn redirect(value: &str) -> Result<Url, AuthError> {
    if value.len() > 2048 {
        return Err(invalid("invalid redirect URI"));
    }
    let u = Url::parse(value).map_err(|_| invalid("invalid redirect URI"))?;
    let local = matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    if !(u.scheme() == "https" || (u.scheme() == "http" && local))
        || u.host_str().is_none()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.query_pairs()
            .any(|(k, _)| matches!(k.as_ref(), "code" | "state" | "error"))
    {
        return Err(invalid(
            "redirect URI requires HTTPS or loopback HTTP, without credentials or fragment",
        ));
    }
    Ok(u)
}
pub(super) fn application(input: &mut ApplicationInput) -> Result<(), AuthError> {
    if !identifier(&input.client_id)
        || !identifier(&input.assistant_id)
        || input.name.trim().is_empty()
        || input.name.len() > 200
        || input.name.chars().any(char::is_control)
        || !(60..=900).contains(&input.access_ttl_seconds)
        || !(300..=super::GRANT_TTL_SECONDS).contains(&input.grant_ttl_seconds)
        || (!input.allow_backend_identity && input.redirect_uris.is_empty())
        || input.redirect_uris.len() > 16
    {
        return Err(invalid("invalid application configuration"));
    }
    for uri in &input.redirect_uris {
        redirect(uri)?;
    }
    input.scopes = scopes(&input.scopes)?;
    Ok(())
}
pub(super) fn authorization(
    input: &AuthorizationRequest,
    app: &ApplicationInput,
) -> Result<Vec<String>, AuthError> {
    let requested = scopes(
        &input
            .scope
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>(),
    )?;
    if !app.enabled
        || !app.redirect_uris.contains(&input.redirect_uri)
        || requested.iter().any(|s| !app.scopes.contains(s))
        || input.response_type != "code"
        || input.code_challenge_method != "S256"
        || input.code_challenge.len() != 43
        || !input
            .code_challenge
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        || input.state.len() < 16
        || input.state.len() > 512
        || input.state.chars().any(char::is_control)
    {
        return Err(invalid("invalid authorization request"));
    }
    Ok(requested)
}
pub fn pkce_challenge(verifier: &str) -> Result<String, AuthError> {
    if !(43..=128).contains(&verifier.len())
        || !verifier
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~'))
    {
        return Err(invalid("invalid PKCE verifier"));
    }
    Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())))
}
