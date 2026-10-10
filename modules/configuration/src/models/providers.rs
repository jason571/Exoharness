// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Provider protocol, URL and secret-reference validation; no network calls.
use super::kinds::model_type;
use crate::{
    ConfigurationError, Result,
    validation::{boolean, fields, text},
};
use serde_json::Value;
use std::collections::BTreeSet;

pub(super) fn defaults(value: &mut Value) -> Result<()> {
    let object = value.as_object_mut().ok_or(ConfigurationError::Json)?;
    for (field, default) in [
        ("model_type", serde_json::json!("llm")),
        ("enabled", serde_json::json!(true)),
        ("api_key_configured", serde_json::json!(false)),
    ] {
        object.entry(field).or_insert(default);
    }
    Ok(())
}

pub(super) fn validate(providers: &[Value]) -> Result<()> {
    let mut provider_names = BTreeSet::new();
    for provider in providers {
        fields(
            provider,
            &[
                "name",
                "model_type",
                "api",
                "output_token_parameter",
                "base_url",
                "secret_env",
                "api_key_configured",
                "enabled",
            ],
        )?;
        let name = text(provider, "name")?;
        if !provider_names.insert(name) {
            return Err(ConfigurationError::Invalid("duplicate provider"));
        }
        let protocol = text(provider, "api")?;
        let kind = model_type(provider)?;
        if !matches!(
            (protocol, kind),
            ("openai_compatible" | "anthropic", "llm")
                | (
                    "volcengine_tts_sse"
                        | "aliyun_qwen_tts_http"
                        | "aliyun_qwen_tts_http_sse"
                        | "aliyun_qwen_audio_tts_websocket",
                    "tts"
                )
                | ("openai_transcription", "asr")
        ) {
            return Err(ConfigurationError::Invalid(
                "protocol and model type mismatch",
            ));
        }
        let uri = text(provider, "base_url")?
            .parse::<http::Uri>()
            .map_err(|_| ConfigurationError::Invalid("invalid provider URL"))?;
        let host = uri
            .host()
            .ok_or(ConfigurationError::Invalid("missing URL host"))?;
        let secure = if protocol == "aliyun_qwen_audio_tts_websocket" {
            "wss"
        } else {
            "https"
        };
        let local = if secure == "wss" { "ws" } else { "http" };
        if uri.scheme_str() != Some(secure)
            && !(uri.scheme_str() == Some(local)
                && matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1"))
            || uri
                .authority()
                .is_some_and(|auth| auth.as_str().contains('@'))
            || text(provider, "base_url")?.contains('#')
        {
            return Err(ConfigurationError::Invalid("provider URL policy rejected"));
        }
        for field in ["enabled", "api_key_configured"] {
            boolean(provider, field)?;
        }
        if let Some(parameter) = provider
            .get("output_token_parameter")
            .filter(|value| !value.is_null())
            && (protocol != "openai_compatible"
                || !matches!(
                    parameter.as_str(),
                    Some("max_tokens" | "max_completion_tokens" | "omit")
                ))
        {
            return Err(ConfigurationError::Invalid(
                "invalid output token parameter",
            ));
        }
        if let Some(env) = provider.get("secret_env").filter(|value| !value.is_null())
            && !env.as_str().is_some_and(|key| {
                !key.is_empty()
                    && key.len() <= 128
                    && key.bytes().all(|byte| {
                        byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
        {
            return Err(ConfigurationError::Invalid("invalid secret reference"));
        }
    }
    Ok(())
}
