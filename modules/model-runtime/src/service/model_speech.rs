//! Callable speech configurations share the runtime lifecycle and revision fence.
use super::ModelRuntime;
use crate::{
    CancellationToken, ErrorKind, SynthesizedAudio,
    errors::{ModelError, Result},
};

#[derive(Clone)]
pub struct SpeechModel {
    pub name: String,
    pub kind: String,
    pub revision: u64,
    pub is_default: bool,
    pub audio_extension: String,
}
impl ModelRuntime {
    pub fn speech_models(&self) -> Result<Vec<SpeechModel>> {
        let catalog = self
            .models
            .refresh()
            .map_err(|_| ModelError::new(ErrorKind::Configuration))?;
        let mut result = Vec::new();
        for model in catalog["models"]
            .as_array()
            .ok_or_else(|| ModelError::new(ErrorKind::Configuration))?
        {
            let Some(name) = model["name"].as_str() else {
                continue;
            };
            let Ok(binding) = self.models.bind_enabled(name) else {
                continue;
            };
            let kind = binding.model["model_type"].as_str().unwrap_or("llm");
            if !matches!(
                (kind, binding.provider["api"].as_str()),
                ("tts", Some("volcengine_tts_sse")) | ("asr", Some("openai_transcription"))
            ) {
                continue;
            }
            result.push(SpeechModel {
                name: name.into(),
                kind: kind.into(),
                revision: binding.revision,
                is_default: catalog["default_models"][kind] == name,
                audio_extension: binding.model["tts"]["audio_format"]
                    .as_str()
                    .unwrap_or("mp3")
                    .into(),
            });
        }
        Ok(result)
    }
    pub async fn synthesize(
        &self,
        name: &str,
        revision: u64,
        text: &str,
        cancel: CancellationToken,
    ) -> Result<SynthesizedAudio> {
        if text.trim().is_empty() || text.len() > self.policy.request_bytes {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        tokio::select! { biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,async {
                let _admission=self.admission.clone().try_acquire_owned().map_err(|_|ModelError::new(ErrorKind::Busy))?;
                let _active=self.active.clone().acquire_owned().await.map_err(|_|ModelError::new(ErrorKind::Busy))?;
                let binding=self.bind(name).await?;
                if binding.revision!=revision { return Err(ModelError::new(ErrorKind::Configuration)); }
                crate::synthesis::run(&self.client,&binding,text,self.policy.request_bytes,self.policy.response_bytes).await
            })=>result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
    pub async fn transcribe_revision(
        &self,
        name: &str,
        revision: u64,
        audio: Vec<u8>,
        mime: &str,
        cancel: CancellationToken,
    ) -> Result<String> {
        crate::transcription::format(mime)?;
        if audio.is_empty() || audio.len() > crate::MAX_AUDIO_BYTES {
            return Err(ModelError::new(ErrorKind::InvalidRequest));
        }
        tokio::select! { biased;
            _=cancel.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            _=self.shutdown.cancelled()=>Err(ModelError::new(ErrorKind::Cancelled)),
            result=tokio::time::timeout(self.policy.timeout,async {
                let _admission=self.admission.clone().try_acquire_owned().map_err(|_|ModelError::new(ErrorKind::Busy))?;
                let _active=self.active.clone().acquire_owned().await.map_err(|_|ModelError::new(ErrorKind::Busy))?;
                let binding=self.bind(name).await?;
                if binding.revision!=revision { return Err(ModelError::new(ErrorKind::Configuration)); }
                crate::transcription::run(&self.client,&binding,audio,mime,self.policy.response_bytes.min(64*1024)).await
            })=>result.unwrap_or_else(|_|Err(ModelError::new(ErrorKind::Timeout))),
        }
    }
}
