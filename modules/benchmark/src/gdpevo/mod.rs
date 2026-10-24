// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! Official dataset boundaries and OCI grading. No host Python interpreter.
mod dataset;
mod gateway;
mod grader;
mod runner;
pub use dataset::Dataset;
pub use gateway::Gateway;
pub use grader::{Grader, normalize_score};
pub use runner::run;
