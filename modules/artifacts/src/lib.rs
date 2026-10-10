//! One permanent signed file delivery boundary; no provider-specific proxy.
mod files;
mod mime;
mod service;
mod signing;
mod types;
pub use service::{Artifacts, RemoteFile, same_service_origin};
pub use types::{ArtifactError, PublishedArtifact};
pub type Result<T> = std::result::Result<T, ArtifactError>;
