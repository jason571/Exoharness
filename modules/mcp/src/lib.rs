//! MCP standard protocol adapters; no service-specific request/result rewriting.
mod admission;
mod client;
mod error;
mod headers;
mod imports;
mod progress;
mod secret_management;
mod service;
pub use client::{Connection, ConnectionOptions};
pub use error::{McpError, Result};
pub use imports::{ImportRequest, parse_import};
pub use progress::Progress;
pub use service::Mcp;
