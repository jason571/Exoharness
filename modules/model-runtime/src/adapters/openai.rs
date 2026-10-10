use super::{WireRequest, base, finish, header, text};
use crate::{
    ChatRequest, Completion, ErrorKind, Finish, Message, ToolCall, ToolChoice, Usage,
    errors::{ModelError, Result},
};
use exoharness_configuration::ModelBinding;
use exoharness_contracts::strict_json;
use serde_json::{Value, json};

pub(super) fn build(binding: &ModelBinding, request: &ChatRequest) -> Result<WireRequest> {
    let messages=request.messages.iter().map(|message|match message {
        Message::System {text}=>json!({"role":"system","content":text}),
        Message::User {text}=>json!({"role":"user","content":text}),
        Message::RuntimeData {text}=>json!({"role":"user","content":text}),
        Message::Assistant {text,calls}=>{
            let mut value=json!({"role":"assistant","content":text});
            if !calls.is_empty() {
                value["tool_calls"]=json!(calls.iter().map(|call|json!({"id":call.id,"type":"function","function":{"name":call.name,"arguments":call.arguments.to_string()}})).collect::<Vec<_>>());
            }
            value
        },
        Message::ToolResult {call_id,content,..}=>json!({"role":"tool","tool_call_id":call_id,"content":content}),
    }).collect::<Vec<_>>();
    let mut body = json!({"model":binding.model["model_id"],"messages":messages,"stream":false});
    // Explicit off must reach the wire, rather than leave a reasoning preset
    // enabled upstream. This is the configured compatible API dialect, not a
    // model-name or host-name exception.
    if binding.model["thinking"] == "none" {
        body["reasoning_effort"] = json!("none");
    }
    let token_field = binding.provider["output_token_parameter"]
        .as_str()
        .unwrap_or("max_tokens");
    // Explicit provider choice: use the upstream default instead of sending
    // an output cap. Absence preserves the existing configured-cap behavior.
    if token_field != "omit" {
        body[token_field] = json!(request.output_token_limit);
    }
    // A configured compatible-protocol extension is an explicit user choice.
    // Pass it through once; an upstream rejection is reported, never retried
    // with silently altered sampling. Do not infer support from a brand/URL.
    for field in ["temperature", "top_p", "top_k"] {
        if let Some(value) = binding.model.get(field).filter(|v| !v.is_null()) {
            body[field] = value.clone();
        }
    }
    if !request.tools.is_empty() {
        body["tools"]=json!(request.tools.iter().map(|tool|json!({"type":"function","function":{"name":tool.name,"description":tool.description,"parameters":tool.input_schema}})).collect::<Vec<_>>());
        body["tool_choice"] = match &request.tool_choice {
            ToolChoice::Auto => json!("auto"),
            ToolChoice::None => json!("none"),
            ToolChoice::Required => json!("required"),
            ToolChoice::Named(name) => json!({"type":"function","function":{"name":name}}),
        };
    }
    let mut headers = reqwest::header::HeaderMap::new();
    binding.with_credential(|key| -> Result<()> {
        if let Some(key) = key {
            headers.insert("authorization", header(&format!("Bearer {key}"))?);
        }
        Ok(())
    })?;
    Ok(WireRequest {
        url: base(binding, "/chat/completions")?,
        headers,
        body,
    })
}
pub(super) fn parse(value: Value) -> Result<Completion> {
    let usage = Usage {
        input_tokens: value["usage"]["prompt_tokens"].as_u64(),
        output_tokens: value["usage"]["completion_tokens"].as_u64(),
        cache_read_tokens: value["usage"]["prompt_tokens_details"]["cached_tokens"].as_u64(),
        cache_write_tokens: None,
    };
    let error = |kind| ModelError::new(kind).with_usage(usage.clone());
    if value.get("error").is_some_and(|v| !v.is_null()) {
        return Err(error(ErrorKind::InvalidResponse));
    }
    let choices = value["choices"]
        .as_array()
        .filter(|v| v.len() == 1)
        .ok_or_else(|| error(ErrorKind::InvalidResponse))?;
    let choice = &choices[0];
    let reason = match choice["finish_reason"].as_str() {
        Some("stop") => Finish::Stop,
        Some("tool_calls") => Finish::ToolCalls,
        Some("length") => return Err(error(ErrorKind::OutputLimit)),
        None => return Err(error(ErrorKind::Incomplete)),
        _ => return Err(error(ErrorKind::InvalidResponse)),
    };
    let message = &choice["message"];
    if message["role"] != "assistant" {
        return Err(error(ErrorKind::InvalidResponse));
    }
    let content = match message.get("content") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        _ => return Err(error(ErrorKind::InvalidResponse)),
    };
    let mut calls = Vec::new();
    if let Some(values) = message.get("tool_calls").filter(|v| !v.is_null()) {
        let values = values
            .as_array()
            .filter(|v| v.len() <= 128)
            .ok_or_else(|| error(ErrorKind::InvalidResponse))?;
        for call in values {
            if call["type"] != "function" {
                return Err(error(ErrorKind::InvalidResponse));
            }
            let parse = || -> Result<ToolCall> {
                Ok(ToolCall {
                    id: text(call, "id")?.into(),
                    name: text(&call["function"], "name")?.into(),
                    arguments: strict_json::parse(text(&call["function"], "arguments")?)
                        .map_err(|_| error(ErrorKind::InvalidResponse))?,
                })
            };
            calls.push(parse().map_err(|_| error(ErrorKind::InvalidResponse))?);
        }
    }
    finish(
        content,
        calls,
        reason,
        usage,
        value["id"].as_str().map(str::to_owned),
    )
}
