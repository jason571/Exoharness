// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::Usage;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Configuration,
    Unsupported,
    InvalidRequest,
    Busy,
    Cancelled,
    Timeout,
    Transport,
    Http,
    ProviderRejected,
    InvalidResponse,
    Incomplete,
    OutputLimit,
    Empty,
}
/// Safe classification only: never provider bodies, URLs, keys or reqwest errors.
#[derive(Debug, thiserror::Error)]
#[error("model request failed: {kind:?}")]
pub struct ModelError {
    pub kind: ErrorKind,
    pub status: Option<u16>,
    pub retry_after_seconds: Option<u64>,
    pub usage: Option<Usage>,
    /// Fixed diagnostic codes only; no response bodies or provider secrets.
    pub response_diagnostic: Option<&'static str>,
    pub binding: Option<Box<serde_json::Value>>,
}
impl ModelError {
    pub(crate) fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            status: None,
            retry_after_seconds: None,
            usage: None,
            response_diagnostic: None,
            binding: None,
        }
    }
    pub(crate) fn with_usage(mut self, usage: Usage) -> Self {
        self.usage = Some(usage);
        self
    }
    pub(crate) fn with_response_diagnostic(mut self, code: &'static str) -> Self {
        self.response_diagnostic = Some(code);
        self
    }
}
pub(crate) type Result<T> = std::result::Result<T, ModelError>;
