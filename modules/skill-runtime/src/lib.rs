//! Environment evidence, separate from import and from executable Tool grants.
//! Preparing never imports a Skill module or runs uploaded installation hooks.
mod cache;
mod metadata;
mod probe;
mod service;
mod types;
mod workers;
pub use service::SkillRuntime;
pub use types::{EnvironmentEvidence, RuntimeError, RuntimeStatus};
pub type Result<T> = std::result::Result<T, RuntimeError>;
