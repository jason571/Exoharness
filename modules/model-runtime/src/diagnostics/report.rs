use exoharness_configuration::ModelBinding;
use serde::Serialize;
#[derive(Debug, Serialize)]
pub struct DiagnosticModel {
    name: String,
    provider: String,
    model_id: String,
    model_type: String,
}
/// No provider body, secret, Base64 audio or raw transcription is serialized.
#[derive(Debug, Serialize)]
pub struct DiagnosticReport {
    pub model: DiagnosticModel,
    pub configuration_revision: u64,
    pub scope: &'static str,
    pub response_id: Option<String>,
    pub usage: Option<crate::Usage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bytes: Option<usize>,
    pub tested: bool,
    pub agent_integrated: bool,
}
impl DiagnosticReport {
    pub(super) fn new(binding: &ModelBinding, scope: &'static str) -> Self {
        let text = |field: &str| binding.model[field].as_str().unwrap_or_default().to_owned();
        Self {
            model: DiagnosticModel {
                name: text("name"),
                provider: text("provider"),
                model_id: text("model_id"),
                model_type: binding.model["model_type"].as_str().unwrap_or("llm").into(),
            },
            configuration_revision: binding.revision,
            scope,
            response_id: None,
            usage: None,
            audio_bytes: None,
            tested: true,
            agent_integrated: false,
        }
    }
}
