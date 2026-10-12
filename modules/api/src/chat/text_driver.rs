// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Streaming model adapter; capability dispatch is a separate provider module.
use super::ChatState;
use crate::errors::storage;
use async_trait::async_trait;
use exoharness_agent::{Driver, EngineState, Receipt, StopReason};
use exoharness_contracts::model::{ChatRequest, Completion, ToolCall};
use exoharness_model_runtime::{CancellationToken, ErrorKind, TextSink};
use serde_json::{Value, json};
use std::{future::Future, pin::Pin};

/// Incremental text is persisted before it becomes visible over SSE. Awaiting
/// the write bounds memory and propagates disconnect/cancel/DB failure upstream.
struct JournalSink {
    state: ChatState,
    owner: String,
    session: String,
    run: String,
    failed: bool,
}
impl TextSink for JournalSink {
    fn push<'a>(
        &'a mut self,
        text: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<(), ()>> + Send + 'a>> {
        Box::pin(async move {
            let owner = self.owner.clone();
            let session = self.session.clone();
            let run = self.run.clone();
            let payload = json!({"type":"text_delta","delta":text});
            let result = storage(self.state.store.clone(), move |s| {
                s.chat_event(&owner, &session, &run, payload)
            })
            .await;
            self.failed |= result.is_err();
            result.map(|_| ()).map_err(|_| ())
        })
    }
}

pub(super) struct TextDriver {
    pub state: ChatState,
    pub owner: String,
    pub session: String,
    pub run: String,
    pub model: String,
    pub revision: u64,
    pub cancel: CancellationToken,
    pub response: Value,
    pub failure: Option<&'static str>,
    pub failed_usage: Option<exoharness_contracts::model::Usage>,
    pub catalog: Option<exoharness_tools::Catalog>,
    pub tool_calls: Vec<String>,
    pub artifacts: Vec<Value>,
}
#[async_trait]
impl Driver for TextDriver {
    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled() || *self.state.stop.borrow()
    }
    fn tools(
        &self,
        fallback: &[exoharness_contracts::model::Tool],
    ) -> Vec<exoharness_contracts::model::Tool> {
        self.catalog
            .as_ref()
            .map_or_else(|| fallback.to_vec(), |c| c.schemas())
    }
    fn requires_execution(&self, requested: bool) -> bool {
        requested
            || self
                .catalog
                .as_ref()
                .is_some_and(|c| c.requires_execution())
    }
    fn request_tools(
        &mut self,
        entries: &[exoharness_core::context::JournalEntry],
        fallback: &[exoharness_contracts::model::Tool],
    ) -> Vec<exoharness_contracts::model::Tool> {
        if let Some(view) = exoharness_agent::workflow_view(entries)
            && let Some(catalog) = self.catalog.as_mut()
            && let Some(mut tools) = catalog.workflow_schemas(&view.tools)
        {
            if super::publication_checkpoint::restrict(entries, &view.invocation, view.required) {
                // Only the saved-file delivery step may run here. Discovery
                // cannot reload a renderer and accidentally execute it twice.
                tools.retain(|tool| view.tools.contains(&tool.name));
            }
            return tools;
        }
        self.tools(fallback)
    }
    async fn model(&mut self, key: &str, request: ChatRequest) -> Result<Completion, StopReason> {
        let sequence: u32 = key
            .strip_prefix("model-")
            .and_then(|s| s.parse().ok())
            .filter(|n| *n > 0 && *n <= 128)
            .ok_or(StopReason::InvalidProposal)?;
        let own = self.owner.clone();
        let sid = self.session.clone();
        let rid = self.run.clone();
        let input = json!(&request);
        storage(self.state.store.clone(), move |s| {
            s.chat_model_start_sequence(&own, &sid, &rid, sequence, &input)
        })
        .await
        .map_err(|_| StopReason::PersistenceFailed)?;
        let mut stop = self.state.stop.clone();
        let mut sink = JournalSink {
            state: self.state.clone(),
            owner: self.owner.clone(),
            session: self.session.clone(),
            run: self.run.clone(),
            failed: false,
        };
        let answer = if self.cancelled() {
            None
        } else {
            tokio::select! {biased;
                _=stop.changed()=>{self.cancel.cancel();None},
                _=self.cancel.cancelled()=>None,
                result=self.state.runtime.complete_revision_stream(&self.model,self.revision,request,self.cancel.clone(),&mut sink)=>Some(result)
            }
        };
        let result = match answer {
            Some(Ok(completion)) => {
                if completion.configuration_revision != self.revision {
                    return Err(StopReason::ModelIncomplete);
                }
                let usage = &completion.usage;
                self.response = json!({"answer":completion.text,"model":{"name":completion.model_name,"provider":completion.provider_name,"model_id":completion.model_id,"api":"chat"},
                    "usage":{"input_tokens":usage.input_tokens,"output_tokens":usage.output_tokens,"total_tokens":usage.input_tokens.zip(usage.output_tokens).map(|(a,b)|a.saturating_add(b)),"cache_read_tokens":usage.cache_read_tokens,"cache_write_tokens":usage.cache_write_tokens},"proposed_calls":completion.calls,"tool_calls":[],"warnings":[],"artifacts":[]});
                Ok(completion)
            }
            Some(Err(error)) => {
                self.failed_usage = error.usage.clone();
                self.response = json!({"model_error":{"kind":format!("{:?}",error.kind),"http_status":error.status,"retry_after_seconds":error.retry_after_seconds},"usage":error.usage.as_ref().map(|u|json!({"input_tokens":u.input_tokens,"output_tokens":u.output_tokens,"total_tokens":u.input_tokens.zip(u.output_tokens).map(|(a,b)|a.saturating_add(b))}))});
                self.failure = Some(match error.kind {
                    ErrorKind::Timeout => "模型请求超时，本次已停止；不会自动重复请求",
                    ErrorKind::Http | ErrorKind::ProviderRejected => {
                        "模型服务拒绝请求，本次已停止，请检查模型配置或限流状态"
                    }
                    ErrorKind::OutputLimit => "模型输出超过上限，本次已停止，没有提交工具执行",
                    ErrorKind::Busy => "模型并发队列已满，请稍后重试",
                    ErrorKind::InvalidResponse => {
                        "模型返回的响应或工具调用无效，本次已停止；该步骤未执行，已有结果已保留"
                    }
                    _ => "模型响应未完成，本次已停止，没有自动重试",
                });
                Err(StopReason::ModelIncomplete)
            }
            None => {
                self.failure = Some("服务停止或运行取消，模型请求已终止");
                Err(StopReason::Cancelled)
            }
        };
        let own = self.owner.clone();
        let sid = self.session.clone();
        let rid = self.run.clone();
        let response = self.response.clone();
        let error = self.failure;
        storage(self.state.store.clone(), move |s| {
            s.chat_model_finish_sequence(&own, &sid, &rid, sequence, &response, error)
        })
        .await
        .map_err(|_| StopReason::PersistenceFailed)?;
        if sink.failed {
            Err(StopReason::PersistenceFailed)
        } else {
            result
        }
    }
    async fn invoke(&mut self, key: &str, call: &ToolCall) -> Result<Receipt, StopReason> {
        super::tool_driver::invoke(self, key, call).await
    }
    async fn record_notice(&mut self, notice: Value) -> Result<(), StopReason> {
        let (owner, session, run) = (self.owner.clone(), self.session.clone(), self.run.clone());
        storage(self.state.store.clone(), move |s| {
            s.chat_event(&owner, &session, &run, notice)
        })
        .await
        .map_err(|_| StopReason::PersistenceFailed)
    }
    async fn save(&mut self, state: &EngineState) -> Result<(), StopReason> {
        let mut metadata =
            serde_json::to_value(state).map_err(|_| StopReason::PersistenceFailed)?;
        // Exact histories live in the request journal, not control metadata.
        metadata
            .as_object_mut()
            .ok_or(StopReason::PersistenceFailed)?
            .remove("entries");
        // Exact proposed arguments are already in execution_model_calls.
        // Cursor metadata must not duplicate potentially large authored files.
        if let Some(pending) = metadata["pending"].as_array_mut() {
            for value in pending {
                value["call"]
                    .as_object_mut()
                    .ok_or(StopReason::PersistenceFailed)?
                    .remove("arguments");
            }
        }
        metadata["journal_reference"] =
            json!({"model_requests":state.model_calls,"session":self.session,"run":self.run});
        let own = self.owner.clone();
        let sid = self.session.clone();
        let rid = self.run.clone();
        storage(self.state.store.clone(), move |s| {
            s.chat_agent_cursor(&own, &sid, &rid, &metadata)
        })
        .await
        .map_err(|_| StopReason::PersistenceFailed)
    }
}
