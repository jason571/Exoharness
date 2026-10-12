// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// One counter per Run and dispatcher, shared across all tool adapters. Values
/// are compared through a canonical native-JSON fingerprint, never a model summary.
#[derive(Debug, Clone, Deserialize)]
pub struct ConsecutiveCalls {
    limit: usize,
    // Accept old cursors, but never duplicate full arguments in new cursors.
    #[serde(default)]
    previous: Option<(String, Value)>,
    #[serde(default)]
    previous_digest: Option<(String, String)>,
    count: usize,
}

impl Serialize for ConsecutiveCalls {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Snapshot {
            limit: usize,
            previous_digest: Option<(String, String)>,
            count: usize,
        }
        let previous_digest = self.previous_digest.clone().or_else(|| {
            self.previous
                .as_ref()
                .map(|(name, args)| (name.clone(), argument_digest(args)))
        });
        Snapshot {
            limit: self.limit,
            previous_digest,
            count: self.count,
        }
        .serialize(serializer)
    }
}

impl ConsecutiveCalls {
    pub fn limit(&self) -> usize {
        self.limit
    }
    /// A model-visible reminder can precede refusal of the next proposal.
    pub fn at_limit(&self) -> bool {
        self.count >= self.limit
    }
    pub fn new(limit: usize) -> Result<Self, &'static str> {
        if limit == 0 {
            return Err("consecutive call limit must be positive");
        }
        Ok(Self {
            limit,
            previous: None,
            previous_digest: None,
            count: 0,
        })
    }

    /// Three identical calls are allowed at limit=3. The fourth is rejected
    /// before dispatch; a different tool or different arguments starts a new
    /// sequence. This does not authorize replay of an external side effect.
    pub fn admit(&mut self, tool: &str, arguments: &Value) -> bool {
        let digest = argument_digest(arguments);
        let same = self
            .previous_digest
            .as_ref()
            .is_some_and(|(name, hash)| name == tool && hash == &digest)
            || self
                .previous
                .as_ref()
                .is_some_and(|(name, args)| name == tool && args == arguments);
        // Upgrade legacy state even when the next attempt is refused.
        self.previous = None;
        self.previous_digest = Some((tool.to_owned(), digest));
        if same {
            if self.count >= self.limit {
                return false;
            }
            self.count += 1;
        } else {
            self.count = 1;
        }
        true
    }
}

fn argument_digest(value: &Value) -> String {
    // Native JSON equality ignores object key order and treats float -0.0 as
    // +0.0, while keeping integer and float representations distinct.
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(fields) => {
                let sorted: std::collections::BTreeMap<_, _> = fields.iter().collect();
                Value::Object(
                    sorted
                        .into_iter()
                        .map(|(k, v)| (k.clone(), canonical(v)))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            Value::Number(n) if n.is_f64() && n.as_f64() == Some(0.0) => serde_json::json!(0.0),
            _ => value.clone(),
        }
    }
    let encoded = serde_json::to_vec(&canonical(value)).expect("JSON values always serialize");
    format!("{:x}", Sha256::digest(encoded))
}
