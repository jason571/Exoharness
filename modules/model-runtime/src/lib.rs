//! Provider-neutral model execution. No tools execute in an adapter.
//! HTTP is asynchronous, bounded, cancellable and has exactly one attempt.
mod adapters;
mod diagnostics;
mod errors;
mod service;
mod streaming;
mod synthesis;
mod transcription;
mod transport;
mod types;
mod validation;
pub use diagnostics::DiagnosticReport;
pub use errors::{ErrorKind, ModelError};
pub use service::model_speech::SpeechModel;
pub use service::{ModelRuntime, RuntimeCapacity, RuntimePolicy};
pub use streaming::TextSink;
pub use synthesis::SynthesizedAudio;
pub use tokio_util::sync::CancellationToken;
pub use transcription::MAX_AUDIO_BYTES;
pub use types::{ChatRequest, Completion, Finish, Message, Tool, ToolCall, ToolChoice, Usage};
