//! A credential-bearing invocation snapshot is never a public projection.
use super::{Models, credentials, persistence, selection};
use crate::{ConfigurationError, Result};
use serde_json::Value;

/// Configuration and key are selected under one lock from one immutable version.
/// Deliberately not Debug/Serialize: credentials must not enter HTTP or logs.
pub struct ModelBinding {
    pub revision: u64,
    pub model: Value,
    pub provider: Value,
    credential: Option<String>,
}
impl ModelBinding {
    pub fn with_credential<T>(&self, action: impl FnOnce(Option<&str>) -> T) -> T {
        action(self.credential.as_deref())
    }
}
impl Models {
    pub fn bind_enabled(&self, name: &str) -> Result<ModelBinding> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| ConfigurationError::PrivateFile)?;
        if let Some(root) = &self.root {
            *state = persistence::load(root)?;
        }
        let index = selection::index(&state.catalog, "models", name)?;
        let model = &state.catalog["models"][index];
        let provider_name = model["provider"].as_str().ok_or(ConfigurationError::Json)?;
        let index = selection::index(&state.catalog, "providers", provider_name)?;
        let provider = &state.catalog["providers"][index];
        if model["enabled"].as_bool() == Some(false) || provider["enabled"].as_bool() == Some(false)
        {
            return Err(ConfigurationError::Invalid("model or provider is disabled"));
        }
        let credential = if provider["api_key_configured"].as_bool() == Some(true) {
            let root = state
                .root
                .as_deref()
                .ok_or(ConfigurationError::PrivateFile)?;
            Some(
                String::from_utf8(credentials::read(root, provider_name)?)
                    .map_err(|_| ConfigurationError::PrivateFile)?,
            )
        } else {
            None
        };
        Ok(ModelBinding {
            revision: state.revision,
            model: model.clone(),
            provider: provider.clone(),
            credential,
        })
    }
}
