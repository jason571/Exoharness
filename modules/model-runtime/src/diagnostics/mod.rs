// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Model diagnostics dispatch by declared type/protocol, not model/service name.
mod asr;
mod report;
mod sse;
mod tts;
use crate::{
    ChatRequest, ErrorKind, Message, ToolChoice, adapters,
    errors::{ModelError, Result},
    transport,
};
use exoharness_configuration::ModelBinding;
pub use report::DiagnosticReport;

pub(crate) async fn run(
    client: &reqwest::Client,
    binding: &ModelBinding,
    request_limit: usize,
    response_limit: usize,
) -> Result<DiagnosticReport> {
    match (
        binding.model["model_type"].as_str().unwrap_or("llm"),
        binding.provider["api"].as_str(),
    ) {
        ("llm", Some("openai_compatible" | "anthropic")) => {
            let request = ChatRequest {
                messages: vec![Message::User {
                    text: "Reply with OK only.".into(),
                }],
                tools: vec![],
                tool_choice: ToolChoice::None,
                output_token_limit: binding.model["max_tokens"].as_u64().unwrap_or(128).min(128),
            };
            crate::validation::request(&request)?;
            let wire = adapters::build(binding, &request)?;
            if serde_json::to_vec(&wire.body)
                .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?
                .len()
                > request_limit
            {
                return Err(ModelError::new(ErrorKind::InvalidRequest));
            }
            let value = transport::send(client, wire, response_limit).await?;
            let completion = adapters::parse(binding, &request, value)?;
            let mut report = DiagnosticReport::new(binding, "chat_completion");
            report.usage = Some(completion.usage);
            report.response_id = completion.response_id;
            Ok(report)
        }
        ("asr", Some("openai_transcription")) => {
            asr::test(client, binding, request_limit, response_limit).await
        }
        ("tts", Some("volcengine_tts_sse")) => {
            tts::test(client, binding, request_limit, response_limit).await
        }
        _ => Err(ModelError::new(ErrorKind::Unsupported)),
    }
}
