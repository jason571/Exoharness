use super::{WireRequest, base, finish, header};
use crate::{
    ChatRequest, Completion, ErrorKind, Finish, Message, ToolCall, ToolChoice, Usage,
    errors::{ModelError, Result},
};
use exoharness_configuration::ModelBinding;
use serde_json::{Value, json};

pub(super) fn build(binding: &ModelBinding, request: &ChatRequest) -> Result<WireRequest> {
    let mut system = Vec::new();
    let mut messages: Vec<Value> = Vec::new();
    for message in &request.messages {
        let (role, blocks) = match message {
            Message::System { text } => {
                system.push(json!({"type":"text","text":text}));
                continue;
            }
            Message::User { text } => ("user", vec![json!({"type":"text","text":text})]),
            Message::RuntimeData { text } => ("user", vec![json!({"type":"text","text":text})]),
            Message::Assistant { text, calls } => {
                let mut blocks = Vec::new();
                if !text.is_empty() {
                    blocks.push(json!({"type":"text","text":text}));
                }
                blocks.extend(calls.iter().map(|call|json!({"type":"tool_use","id":call.id,"name":call.name,"input":call.arguments})));
                ("assistant", blocks)
            }
            Message::ToolResult {
                call_id,
                content,
                is_error,
            } => (
                "user",
                vec![
                    json!({"type":"tool_result","tool_use_id":call_id,"content":content,"is_error":is_error}),
                ],
            ),
        };
        // Multiple results for one proposal form a single native user message.
        if let Some(previous) = messages.last_mut().filter(|value| value["role"] == role) {
            previous["content"]
                .as_array_mut()
                .ok_or_else(|| ModelError::new(ErrorKind::InvalidRequest))?
                .extend(blocks);
        } else {
            messages.push(json!({"role":role,"content":blocks}));
        }
    }
    let mut body = json!({"model":binding.model["model_id"],"messages":messages,"max_tokens":request.output_token_limit,"stream":false});
    if binding.model["thinking"] == "none" {
        body["thinking"] = json!({"type":"disabled"});
    }
    if !system.is_empty() {
        body["system"] = json!(system);
    }
    for field in ["temperature", "top_p", "top_k"] {
        if let Some(value) = binding.model.get(field).filter(|v| !v.is_null()) {
            body[field] = value.clone();
        }
    }
    if !request.tools.is_empty() && !matches!(request.tool_choice, ToolChoice::None) {
        body["tools"]=json!(request.tools.iter().map(|tool|json!({"name":tool.name,"description":tool.description,"input_schema":tool.input_schema})).collect::<Vec<_>>());
        body["tool_choice"] = match &request.tool_choice {
            ToolChoice::Auto => json!({"type":"auto"}),
            ToolChoice::Required => json!({"type":"any"}),
            ToolChoice::Named(name) => json!({"type":"tool","name":name}),
            ToolChoice::None => unreachable!(),
        };
    }
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        "anthropic-version",
        reqwest::header::HeaderValue::from_static("2023-06-01"),
    );
    binding.with_credential(|key| -> Result<()> {
        if let Some(key) = key {
            headers.insert("x-api-key", header(key)?);
        }
        Ok(())
    })?;
    Ok(WireRequest {
        url: base(binding, "/messages")?,
        headers,
        body,
    })
}
pub(super) fn parse(value: Value) -> Result<Completion> {
    let usage = Usage {
        input_tokens: value["usage"]["input_tokens"].as_u64(),
        output_tokens: value["usage"]["output_tokens"].as_u64(),
        cache_read_tokens: value["usage"]["cache_read_input_tokens"].as_u64(),
        cache_write_tokens: value["usage"]["cache_creation_input_tokens"].as_u64(),
    };
    let error = |kind| ModelError::new(kind).with_usage(usage.clone());
    if value["type"] != "message" || value["role"] != "assistant" {
        return Err(error(ErrorKind::InvalidResponse));
    }
    let reason = match value["stop_reason"].as_str() {
        Some("end_turn" | "stop_sequence") => Finish::Stop,
        Some("tool_use") => Finish::ToolCalls,
        Some("max_tokens") => return Err(error(ErrorKind::OutputLimit)),
        None => return Err(error(ErrorKind::Incomplete)),
        _ => return Err(error(ErrorKind::InvalidResponse)),
    };
    let content = value["content"]
        .as_array()
        .filter(|v| v.len() <= 256)
        .ok_or_else(|| error(ErrorKind::InvalidResponse))?;
    let mut text = String::new();
    let mut calls = Vec::new();
    for block in content {
        match block["type"].as_str() {
            Some("text") => text.push_str(
                block["text"]
                    .as_str()
                    .ok_or_else(|| error(ErrorKind::InvalidResponse))?,
            ),
            Some("tool_use") => calls.push(ToolCall {
                id: block["id"]
                    .as_str()
                    .ok_or_else(|| error(ErrorKind::InvalidResponse))?
                    .into(),
                name: block["name"]
                    .as_str()
                    .ok_or_else(|| error(ErrorKind::InvalidResponse))?
                    .into(),
                arguments: block["input"].clone(),
            }),
            _ => return Err(error(ErrorKind::InvalidResponse)),
        }
    }
    finish(
        text,
        calls,
        reason,
        usage,
        value["id"].as_str().map(str::to_owned),
    )
}
