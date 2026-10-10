//! Provider-neutral context policy. HTTP routes and persistence are separate.
mod catalog_context;
mod checkpoint_reads;
mod context;
mod engine;
mod input_correction;
mod ports;
mod projection;
mod state;
mod workflow;
pub use context::text_request;
pub use engine::run;
pub use ports::{Driver, DurableReference, Receipt, ReceiptState};
pub use projection::project_request;
pub use state::{EnginePolicy, EngineState, PendingCall, RunOutcome, StopReason};
pub use workflow::workflow_view;
