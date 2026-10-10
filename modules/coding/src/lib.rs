// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Software-engineering policies. No host shell, HTTP, model or filesystem I/O.
//! Persistence/sandbox adapters must independently enforce containment and CAS.
pub mod edits;
pub mod patch;
pub mod paths;
pub mod search;
