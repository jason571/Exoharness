// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Validate complete history before making any network request.
use crate::{
    ChatRequest, ErrorKind, Message, ToolChoice,
    errors::{ModelError, Result},
};
use std::collections::BTreeSet;
pub(crate) fn name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
pub(crate) fn request(value: &ChatRequest) -> Result<()> {
    let bad = || ModelError::new(ErrorKind::InvalidRequest);
    if value.messages.is_empty()
        || value.messages.len() > 4096
        || value.tools.len() > 128
        || value.output_token_limit == 0
    {
        return Err(bad());
    }
    let mut tools = BTreeSet::new();
    for tool in &value.tools {
        if !name(&tool.name) || !tools.insert(&tool.name) || !tool.input_schema.is_object() {
            return Err(bad());
        }
    }
    match &value.tool_choice {
        ToolChoice::Required if tools.is_empty() => return Err(bad()),
        ToolChoice::Named(name) if !tools.contains(name) => return Err(bad()),
        _ => {}
    }
    let mut pending = BTreeSet::new();
    let mut ids = BTreeSet::new();
    let mut non_system = false;
    for message in &value.messages {
        match message {
            Message::ToolResult { call_id, .. } => {
                if !pending.remove(call_id) {
                    return Err(bad());
                }
            }
            _ if !pending.is_empty() => return Err(bad()),
            Message::System { text } => {
                if non_system || text.trim().is_empty() {
                    return Err(bad());
                }
            }
            Message::Assistant { text, calls } => {
                non_system = true;
                if text.trim().is_empty() && calls.is_empty() {
                    return Err(bad());
                }
                for call in calls {
                    if call.id.is_empty()
                        || call.id.len() > 256
                        || !ids.insert(&call.id)
                        || !name(&call.name)
                        || !call.arguments.is_object()
                    {
                        return Err(bad());
                    }
                    pending.insert(&call.id);
                }
            }
            Message::User { text } | Message::RuntimeData { text } => {
                non_system = true;
                if text.trim().is_empty() {
                    return Err(bad());
                }
            }
        }
    }
    if !pending.is_empty() || !non_system {
        return Err(bad());
    }
    Ok(())
}
