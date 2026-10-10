// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Model providers join the same authorized, lazy tool catalog as MCP providers.
use crate::{Binding, Catalog, DispatchResult, Result, ToolError, ToolRuntime, admission};
use exoharness_contracts::model::Tool;
use exoharness_model_runtime::{CancellationToken, ErrorKind, SpeechModel};
use serde_json::{Value, json};
use std::io::{Read, Write};

impl Catalog {
    pub(crate) fn add_models(&mut self, models: Vec<SpeechModel>) {
        for model in models {
            if admission::allowed(&self.profile, "models", &model.name) {
                self.model_choices.insert(model.name.clone(), model);
            }
        }
        for (kind, name, raw, description) in [
            (
                "tts",
                "model_synthesize_speech",
                "model.synthesize_speech",
                "Synthesize speech / TTS 配音 using an enabled authorized configured model. Returns a persisted audio artifact, never base64. Default speaker and audio format come from model configuration. Internal narration may be retained without displaying an intermediate audio card.",
            ),
            (
                "asr",
                "model_transcribe_audio",
                "model.transcribe_audio",
                "Transcribe speech / ASR 语音转文字 from this user's persisted audio artifact. No arbitrary URL, host path or base64 input; uses enabled authorized configured recognition models.",
            ),
        ] {
            let choices: Vec<_> = self
                .model_choices
                .values()
                .filter(|m| m.kind == kind)
                .map(|m| m.name.clone())
                .collect();
            if choices.is_empty() {
                continue;
            }
            let mut properties = json!({"model":{"type":"string","enum":choices}});
            let required = if kind == "tts" {
                properties["text"] = json!({"type":"string","minLength":1,"maxLength":32768});
                properties["filename"] = json!({"type":"string","pattern":"^[^./\\\\:\"%?#\\u0000-\\u001f][^/\\\\:\"%?#\\u0000-\\u001f]*\\.(mp3|wav)$","maxLength":240});
                properties["artifact_visibility"] =
                    json!({"type":"string","enum":["internal","public"],"default":"public"});
                vec!["text"]
            } else {
                properties["artifact_id"] =
                    json!({"type":"string","pattern":"^artifact-[a-f0-9]{64}$"});
                vec!["artifact_id"]
            };
            let tool = Tool {
                name: name.into(),
                description: description.into(),
                input_schema: json!({"type":"object","additionalProperties":false,"properties":properties,"required":required}),
            };
            // A group revision is the configuration revision, not a fake registry row.
            let revision = self
                .model_choices
                .values()
                .find(|m| m.kind == kind)
                .unwrap()
                .revision;
            self.bindings.insert(
                name.into(),
                Binding {
                    capability: format!("tool:model:{kind}"),
                    revision,
                    raw_name: raw.into(),
                    tool,
                },
            );
        }
    }
    fn selected_model(&self, kind: &str, args: &Value) -> Result<&SpeechModel> {
        let model = if let Some(name) = args["model"].as_str() {
            self.model_choices.get(name).filter(|m| m.kind == kind)
        } else {
            self.model_choices
                .values()
                .find(|m| m.kind == kind && m.is_default)
                .or_else(|| self.model_choices.values().find(|m| m.kind == kind))
        };
        model.ok_or(ToolError::Authorization)
    }
}
impl ToolRuntime {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn run_model(
        &self,
        catalog: &Catalog,
        owner: &str,
        session: &str,
        run: &str,
        args: &Value,
        binding: &Binding,
        cancel: CancellationToken,
    ) -> Result<DispatchResult> {
        let runtime = self
            .model_runtime
            .as_ref()
            .ok_or(ToolError::Authorization)?;
        let kind = binding
            .capability
            .strip_prefix("tool:model:")
            .ok_or(ToolError::Authorization)?;
        let model = catalog.selected_model(kind, args)?.clone();
        if !admission::allowed(&catalog.profile, "models", &model.name) {
            return Err(ToolError::Authorization);
        }
        if kind == "tts" {
            let filename = args["filename"]
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| format!("speech.{}", model.audio_extension));
            if !filename.ends_with(&format!(".{}", model.audio_extension)) {
                return Err(ToolError::Input(
                    "文件扩展名与配置的音频格式不同；未发送模型请求".into(),
                ));
            }
            let audio = runtime
                .synthesize(
                    &model.name,
                    model.revision,
                    args["text"].as_str().unwrap_or(""),
                    cancel,
                )
                .await
                .map_err(model_error)?;
            let service = self.artifacts.clone();
            let (owner, session, run) = (owner.to_owned(), session.to_owned(), run.to_owned());
            let published = tokio::task::spawn_blocking(move || {
                let mut file = tempfile::tempfile().map_err(|_| ToolError::Unknown)?;
                file.write_all(&audio.bytes)
                    .map_err(|_| ToolError::Unknown)?;
                // publish_file reads from the start itself through files::save.
                use std::io::{Seek, SeekFrom};
                file.seek(SeekFrom::Start(0))
                    .map_err(|_| ToolError::Unknown)?;
                service
                    .publish_file(&owner, &session, &run, &filename, file)
                    .map_err(|_| ToolError::Unknown)
            })
            .await
            .map_err(|_| ToolError::Unknown)??;
            let mut card =
                serde_json::to_value(&published.artifact).map_err(|_| ToolError::Unknown)?;
            card["url"] = json!(published.url);
            card["download_url"] = json!(published.download_url);
            card["kind"] = json!("audio");
            let internal = args["artifact_visibility"] == "internal";
            let value = json!({"status":"completed","model":model.name,"configuration_revision":model.revision,"artifact_id":published.artifact.id,"audio":card,"artifact_visibility":if internal{"internal"}else{"public"}});
            Ok(DispatchResult {
                value,
                executed: true,
                failed: false,
                artifacts: if internal { vec![] } else { vec![card] },
            })
        } else if kind == "asr" {
            let artifacts = self.artifacts.clone();
            let own = owner.to_owned();
            let id = args["artifact_id"]
                .as_str()
                .ok_or(ToolError::Authorization)?
                .to_owned();
            let (record, bytes) = tokio::task::spawn_blocking(move || {
                let (record, mut file) = artifacts
                    .open_owned(&own, &id)
                    .map_err(|_| ToolError::Authorization)?;
                if !record.content_type.starts_with("audio/")
                    || record.size_bytes <= 0
                    || record.size_bytes > exoharness_model_runtime::MAX_AUDIO_BYTES as i64
                {
                    return Err(ToolError::Input(
                        "需要当前用户可访问、大小允许的音频文件；未发送模型请求".into(),
                    ));
                }
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)
                    .map_err(|_| ToolError::Storage)?;
                Ok((record, bytes))
            })
            .await
            .map_err(|_| ToolError::Storage)??;
            let text = runtime
                .transcribe_revision(
                    &model.name,
                    model.revision,
                    bytes,
                    &record.content_type,
                    cancel,
                )
                .await
                .map_err(model_error)?;
            Ok(DispatchResult {
                value: json!({"status":"completed","model":model.name,"configuration_revision":model.revision,"artifact_id":record.id,"text":text}),
                executed: true,
                failed: false,
                artifacts: vec![],
            })
        } else {
            Err(ToolError::Authorization)
        }
    }
}
fn model_error(error: exoharness_model_runtime::ModelError) -> ToolError {
    match error.kind {
        ErrorKind::Configuration
        | ErrorKind::Unsupported
        | ErrorKind::InvalidRequest
        | ErrorKind::Busy => ToolError::Input(
            "语音模型未就绪、配置版本已变化或参数不受支持；未提交生成，请核对模型配置".into(),
        ),
        _ => ToolError::Unknown,
    }
}
