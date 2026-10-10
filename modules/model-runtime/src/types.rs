// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Compatibility re-export; shared request/response contracts belong to Core boundaries.
pub use exoharness_contracts::model::{
    ChatRequest, Completion, Finish, Message, Tool, ToolCall, ToolChoice, Usage,
};
