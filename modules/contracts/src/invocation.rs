// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use serde::{Deserialize, Serialize};

/// An unknown side effect is not a failed side effect and cannot be retried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvocationState {
    Prepared,
    Submitted,
    Succeeded,
    RejectedBeforeExecution,
    FailedAfterExecution,
    OutcomeUnknown,
}

impl InvocationState {
    /// This checks state only. Dispatch must also atomically claim the stable
    /// invocation ID and verify the frozen schema, grant and revision snapshot.
    pub fn may_submit(self) -> bool {
        matches!(self, Self::Prepared | Self::RejectedBeforeExecution)
    }
}
