//! Protocol dispatch is by configured API, never provider or model name.
mod anthropic;
mod openai;
use crate::{
    ChatRequest, Completion, ErrorKind,
    errors::{ModelError, Result},
};
use exoharness_configuration::ModelBinding;
use serde_json::Value;
pub(crate) struct WireRequest {
    pub url: String,
    pub headers: reqwest::header::HeaderMap,
    pub body: Value,
}
pub(crate) fn build(binding: &ModelBinding, request: &ChatRequest) -> Result<WireRequest> {
    if binding.model["model_type"].as_str().unwrap_or("llm") != "llm" {
        return Err(ModelError::new(ErrorKind::Unsupported));
    }
    // Existing configuration has no per-protocol reasoning budget. Do not guess
    // a model-specific reasoning field or silently discard an explicit choice.
    if binding.model["thinking"]
        .as_str()
        .is_some_and(|mode| mode != "none")
    {
        return Err(ModelError::new(ErrorKind::Unsupported));
    }
    match binding.provider["api"].as_str() {
        Some("openai_compatible") => openai::build(binding, request),
        Some("anthropic") => anthropic::build(binding, request),
        _ => Err(ModelError::new(ErrorKind::Unsupported)),
    }
}
pub(crate) fn parse(
    binding: &ModelBinding,
    request: &ChatRequest,
    value: Value,
) -> Result<Completion> {
    let mut output = match binding.provider["api"].as_str() {
        Some("openai_compatible") => openai::parse(value),
        Some("anthropic") => anthropic::parse(value),
        _ => Err(ModelError::new(ErrorKind::Unsupported)),
    }?;
    let bad = || ModelError::new(ErrorKind::InvalidResponse).with_usage(output.usage.clone());
    let mut ids = request
        .messages
        .iter()
        .filter_map(|message| match message {
            crate::Message::Assistant { calls, .. } => Some(calls.iter().map(|call| &call.id)),
            _ => None,
        })
        .flatten()
        .collect::<std::collections::BTreeSet<_>>();
    for call in &output.calls {
        if !ids.insert(&call.id)
            || !crate::validation::name(&call.name)
            || !request.tools.iter().any(|tool| tool.name == call.name)
        {
            return Err(bad());
        }
    }
    match &request.tool_choice {
        crate::ToolChoice::None if !output.calls.is_empty() => return Err(bad()),
        crate::ToolChoice::Required if output.calls.is_empty() => return Err(bad()),
        crate::ToolChoice::Named(name)
            if output.calls.is_empty() || output.calls.iter().any(|call| &call.name != name) =>
        {
            return Err(bad());
        }
        _ => {}
    }
    output.configuration_revision = binding.revision;
    output.model_name = binding.model["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    output.provider_name = binding.provider["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    output.model_id = binding.model["model_id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    Ok(output)
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ModelError::new(ErrorKind::InvalidResponse))
}
fn header(value: &str) -> Result<reqwest::header::HeaderValue> {
    let mut header = reqwest::header::HeaderValue::from_str(value)
        .map_err(|_| ModelError::new(ErrorKind::Configuration))?;
    header.set_sensitive(true);
    Ok(header)
}
fn base(binding: &ModelBinding, path: &str) -> Result<String> {
    let base = binding.provider["base_url"]
        .as_str()
        .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?;
    let url = reqwest::Url::parse(base).map_err(|_| ModelError::new(ErrorKind::Configuration))?;
    if url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(ModelError::new(ErrorKind::Configuration));
    }
    Ok(format!("{}{}", base.trim_end_matches('/'), path))
}
fn finish(
    text: String,
    calls: Vec<crate::ToolCall>,
    finish: crate::Finish,
    usage: crate::Usage,
    id: Option<String>,
) -> Result<Completion> {
    let error = |kind| ModelError::new(kind).with_usage(usage.clone());
    if text.trim().is_empty() && calls.is_empty() {
        return Err(error(ErrorKind::Empty));
    }
    if (finish == crate::Finish::ToolCalls) != !calls.is_empty() {
        return Err(error(ErrorKind::InvalidResponse));
    }
    for call in &calls {
        if call.id.is_empty() || call.id.len() > 256 || !call.arguments.is_object() {
            return Err(error(ErrorKind::InvalidResponse));
        }
    }
    Ok(Completion {
        response_id: id,
        model_name: String::new(),
        provider_name: String::new(),
        model_id: String::new(),
        configuration_revision: 0,
        text,
        calls,
        finish,
        usage,
    })
}
