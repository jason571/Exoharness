// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
//! One diagnostic-led correction of an entirely unsubmitted model proposal.
use exoharness_contracts::model::Usage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelCorrection {
    ToolArgumentsJson,
    ToolNotLoaded,
    CallIdReused,
    ArgumentsNotObject,
}
#[derive(Debug, Clone)]
pub struct FailedModel {
    pub usage: Usage,
    pub correction: Option<ModelCorrection>,
}
impl ModelCorrection {
    pub(crate) fn guidance(self) -> &'static str {
        match self {
            Self::ToolArgumentsJson => {
                "The last model proposal had invalid or duplicate-key JSON tool arguments. No call from that response was submitted. Correct the JSON encoding, keep files in small coherent writes when needed, and preserve all user requirements. Use structured objects, proper escaping and unique keys."
            }
            Self::ToolNotLoaded => {
                "The last model proposal named a tool outside the currently loaded schemas. No call from that response was submitted. Inspect the loaded tool names; use authorized discovery and activate_tools to load the exact required interface before proposing execution. Do not invent names or assume unavailable permissions."
            }
            Self::CallIdReused => {
                "The last model proposal reused an existing tool-call ID. No call from that response was submitted. Use new unique call IDs for genuinely new work. Reuse existing successful receipts; do not repeat completed side effects."
            }
            Self::ArgumentsNotObject => {
                "The last model proposal's tool arguments were not a JSON object. No call from that response was submitted. Supply the actual object required by the live schema, not a quoted JSON string or array."
            }
        }
    }
}
