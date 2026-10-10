//! Linux OCI execution backend. It never interprets host shell source, pulls
//! images or falls back to an unconfined process. Authorization is upstream.
mod engine;
mod filesystem;
mod job;
mod output;
mod service;
mod task;
mod types;
pub use job::JobHandle;
pub use service::Sandbox;
pub use task::{TaskEdit, TaskExecution, TaskFile, TaskPatch, TaskSnapshot, TaskStore};
pub use types::{Limits, Receipt, RunSpec, Runtime, SandboxError, Workspace};
pub type Result<T> = std::result::Result<T, SandboxError>;
