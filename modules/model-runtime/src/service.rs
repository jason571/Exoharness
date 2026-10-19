// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    CancellationToken, ChatRequest, Completion, ErrorKind, adapters,
    errors::{ModelError, Result},
    transport, validation,
};
use exoharness_configuration::{ModelBinding, Models};
use exoharness_core::services::Service;
use std::{sync::Arc, time::Duration};
use tokio::sync::Semaphore;
pub(crate) mod model_speech;
mod speech;

#[derive(Debug, Clone)]
pub struct RuntimePolicy {
    pub timeout: Duration,
    pub concurrency: usize,
    pub queued: usize,
    pub request_bytes: usize,
    pub response_bytes: usize,
}
impl Default for RuntimePolicy {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(600),
            concurrency: 4,
            queued: 16,
            request_bytes: 2 * 1024 * 1024,
            response_bytes: 8 * 1024 * 1024,
        }
    }
}
pub struct ModelRuntime {
    models: Arc<Models>,
    client: reqwest::Client,
    policy: RuntimePolicy,
    active: Arc<Semaphore>,
    admission: Arc<Semaphore>,
    bindings: Arc<Semaphore>,
    shutdown: CancellationToken,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCapacity {
    pub active: usize,
    pub queued: usize,
    pub closed: bool,
}
impl Service for ModelRuntime {
    const KEY: &'static str = "model-runtime";
}
impl ModelRuntime {
    /// Instantaneous diagnostic, not a promise of future admission.
    pub fn capacity(&self) -> RuntimeCapacity {
        let active = self.policy.concurrency - self.active.available_permits();
        let admitted =
            self.policy.concurrency + self.policy.queued - self.admission.available_permits();
        RuntimeCapacity {
            active,
            queued: admitted.saturating_sub(active),
            closed: self.shutdown.is_cancelled(),
        }
    }
    pub fn new(models: Arc<Models>, policy: RuntimePolicy) -> Result<Self> {
        if policy.timeout.is_zero()
            || policy.timeout > Duration::from_secs(3600)
            || !(1..=64).contains(&policy.concurrency)
            || policy.queued > 1024
            || !(1024..=16 * 1024 * 1024).contains(&policy.request_bytes)
            || !(1024..=32 * 1024 * 1024).contains(&policy.response_bytes)
        {
            return Err(ModelError::new(ErrorKind::Configuration));
        }
        Ok(Self {
            models,
            client: transport::client()?,
            active: Arc::new(Semaphore::new(policy.concurrency)),
            admission: Arc::new(Semaphore::new(policy.concurrency + policy.queued)),
            bindings: Arc::new(Semaphore::new(policy.concurrency)),
            shutdown: CancellationToken::new(),
            policy,
        })
    }
    /// Cancellation/deadline includes queue, private snapshot lookup and HTTP.
    /// An adapter only returns proposals. Authorization/Schema/idempotency and
    /// dispatch remain the Agent's responsibility, never a provider callback.
    pub async fn complete(
        &self,
        name: &str,
        request: ChatRequest,
        cancel: CancellationToken,
    ) -> Result<Completion> {
        tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,self.once(name,request,None))=>
                result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
    /// Run dispatch pins a catalog generation. A changed binding is rejected
    /// before constructing or submitting a provider request.
    pub async fn complete_revision(
        &self,
        name: &str,
        revision: u64,
        request: ChatRequest,
        cancel: CancellationToken,
    ) -> Result<Completion> {
        tokio::select! {biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,self.once(name,request,Some(revision)))=>result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
    /// Core disposal cancels all in-flight/queued operations and waits for owned
    /// HTTP admissions and any non-cancellable private-file lookup to finish.
    pub async fn close(&self) {
        self.shutdown.cancel();
        let _active = self
            .active
            .acquire_many(self.policy.concurrency as u32)
            .await;
        let _bindings = self
            .bindings
            .acquire_many(self.policy.concurrency as u32)
            .await;
    }
    /// Explicit administrator diagnostic. Types choose their own real protocol,
    /// sharing the same admission/deadline/cancellation and immutable binding.
    pub async fn test(
        &self,
        name: &str,
        cancel: CancellationToken,
    ) -> Result<crate::DiagnosticReport> {
        tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,self.test_once(name))=>
                result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
    async fn test_once(&self, name: &str) -> Result<crate::DiagnosticReport> {
        let _admission = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ModelError::new(ErrorKind::Busy))?;
        let _active = self
            .active
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ModelError::new(ErrorKind::Busy))?;
        let binding = self.bind(name).await?;
        crate::diagnostics::run(
            &self.client,
            &binding,
            self.policy.request_bytes,
            self.policy.response_bytes,
        )
        .await
    }
    async fn once(
        &self,
        name: &str,
        request: ChatRequest,
        revision: Option<u64>,
    ) -> Result<Completion> {
        self.once_with_sink(name, request, revision, None).await
    }
    async fn once_with_sink(
        &self,
        name: &str,
        request: ChatRequest,
        revision: Option<u64>,
        sink: Option<&mut dyn crate::TextSink>,
    ) -> Result<Completion> {
        validation::request(&request)?;
        // Bound before cloning/building protocol-specific messages.
        if serde_json::to_vec(&request)
            .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?
            .len()
            > self.policy.request_bytes
        {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        let _admission = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| ModelError::new(ErrorKind::Busy))?;
        let _active = self
            .active
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ModelError::new(ErrorKind::Busy))?;
        let binding = self.bind(name).await?;
        if revision.is_some_and(|expected| binding.revision != expected) {
            return Err(ModelError::new(ErrorKind::Configuration));
        }
        self.chat_with_sink(&binding, &request, sink)
            .await
            .map_err(|mut error| {
                error.binding = Some(Box::new(serde_json::json!({"name":binding.model["name"],
                "provider":binding.provider["name"],"model_id":binding.model["model_id"],
                "configuration_revision":binding.revision,"api":"chat"})));
                error
            })
    }
    async fn bind(&self, name: &str) -> Result<ModelBinding> {
        let binding_permit = self
            .bindings
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ModelError::new(ErrorKind::Busy))?;
        let models = self.models.clone();
        let name = name.to_owned();
        tokio::task::spawn_blocking(move || {
            let _permit = binding_permit;
            models.bind_enabled(&name)
        })
        .await
        .map_err(|_| ModelError::new(ErrorKind::Configuration))?
        .map_err(|_| ModelError::new(ErrorKind::Configuration))
    }
    async fn chat_with_sink(
        &self,
        binding: &ModelBinding,
        request: &ChatRequest,
        sink: Option<&mut dyn crate::TextSink>,
    ) -> Result<Completion> {
        if binding.model["max_tokens"]
            .as_u64()
            .is_some_and(|limit| request.output_token_limit > limit)
        {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        let mut wire = adapters::build(binding, request)?;
        if sink.is_some() {
            wire.body["stream"] = serde_json::json!(true);
            if binding.provider["api"] == "openai_compatible" {
                wire.body["stream_options"] = serde_json::json!({"include_usage":true});
            }
        }
        if serde_json::to_vec(&wire.body)
            .map_err(|_| ModelError::new(ErrorKind::InvalidRequest))?
            .len()
            > self.policy.request_bytes
        {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        let response = if let Some(sink) = sink {
            crate::streaming::send(
                &self.client,
                wire,
                binding.provider["api"].as_str().unwrap_or_default(),
                self.policy.response_bytes,
                sink,
            )
            .await?
        } else {
            transport::send(&self.client, wire, self.policy.response_bytes).await?
        };
        adapters::parse(binding, request, response)
    }

    /// Incremental answer text, with backpressure and the same pinned binding,
    /// cancellation, deadline and admission as buffered completion. Tool calls
    /// remain private until the complete response has passed native validation.
    pub async fn complete_revision_stream(
        &self,
        name: &str,
        revision: u64,
        request: ChatRequest,
        cancel: CancellationToken,
        sink: &mut dyn crate::TextSink,
    ) -> Result<Completion> {
        tokio::select! {biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,self.once_with_sink(name,request,Some(revision),Some(sink)))=>result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
}
