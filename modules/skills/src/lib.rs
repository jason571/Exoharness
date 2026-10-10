// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Immutable Skill import and progressive reads. No uploaded code executes here.
mod archive;
mod errors;
mod metadata;
mod normalization;
mod package;
mod preview;
mod service;
mod snapshot;
mod storage;
mod types;
pub use errors::{Result, SkillError};
pub use package::decode;
pub use service::Skills;
pub use snapshot::PackageSnapshot;
pub use types::{ImportRequest, Inspection, PackageFile, Preview};
