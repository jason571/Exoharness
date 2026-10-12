// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Provider-neutral context policy. HTTP routes and persistence are separate.
mod catalog_context;
mod checkpoint_reads;
mod context;
mod engine;
mod input_correction;
mod media_context;
mod ports;
mod projection;
mod read_loop;
mod recovery_budget;
mod repair_context;
mod sandbox_recovery;
mod service_recovery;
mod state;
mod workflow;
pub use context::text_request;
pub use engine::run;
pub use ports::{Driver, DurableReference, Receipt, ReceiptState};
pub use projection::project_request;
pub use state::{EnginePolicy, EngineState, PendingCall, RunOutcome, StopReason};
pub use workflow::workflow_view;
