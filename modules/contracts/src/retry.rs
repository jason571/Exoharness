// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One counter per Run and dispatcher, shared across all tool adapters. Values
/// are compared as native JSON, not strings or model-produced summaries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsecutiveCalls {
    limit: usize,
    previous: Option<(String, Value)>,
    count: usize,
}

impl ConsecutiveCalls {
    pub fn limit(&self) -> usize {
        self.limit
    }
    pub fn new(limit: usize) -> Result<Self, &'static str> {
        if limit == 0 {
            return Err("consecutive call limit must be positive");
        }
        Ok(Self {
            limit,
            previous: None,
            count: 0,
        })
    }

    /// Three identical calls are allowed at limit=3. The fourth is rejected
    /// before dispatch; a different tool or different arguments starts a new
    /// sequence. This does not authorize replay of an external side effect.
    pub fn admit(&mut self, tool: &str, arguments: &Value) -> bool {
        if self
            .previous
            .as_ref()
            .is_some_and(|(name, args)| name == tool && args == arguments)
        {
            if self.count >= self.limit {
                return false;
            }
            self.count += 1;
        } else {
            self.previous = Some((tool.to_owned(), arguments.clone()));
            self.count = 1;
        }
        true
    }
}
