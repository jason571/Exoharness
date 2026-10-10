// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! User speech shares model lifecycle/admission but never the LLM adapter.
use super::ModelRuntime;
use crate::{
    CancellationToken, ErrorKind,
    errors::{ModelError, Result},
};
use std::time::Duration;
impl ModelRuntime {
    pub async fn transcription_available(
        &self,
        name: &str,
        cancel: CancellationToken,
    ) -> Result<()> {
        tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(Duration::from_secs(3), self.speech_once(name, None))=>
                result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))).map(|_|()),
        }
    }
    pub async fn transcribe(
        &self,
        name: &str,
        audio: Vec<u8>,
        mime: &str,
        cancel: CancellationToken,
    ) -> Result<String> {
        crate::transcription::format(mime)?;
        if audio.is_empty() || audio.len() > crate::MAX_AUDIO_BYTES {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout.min(Duration::from_secs(120)), self.speech_once(name, Some((audio, mime))))=>
                result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
    async fn speech_once(&self, name: &str, audio: Option<(Vec<u8>, &str)>) -> Result<String> {
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
        match audio {
            Some((audio, mime)) => {
                crate::transcription::run(
                    &self.client,
                    &binding,
                    audio,
                    mime,
                    self.policy.response_bytes.min(64 * 1024),
                )
                .await
            }
            None => {
                crate::transcription::probe(&self.client, &binding).await?;
                Ok(String::new())
            }
        }
    }
}
