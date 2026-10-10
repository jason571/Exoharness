// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Generic concurrency configuration; registration does not grant execution.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourcePool {
    pub id: String,
    pub name: String,
    pub max_concurrency: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchedulerConfiguration {
    pub revision: u64,
    pub max_active_runs_per_user: u32,
    pub max_global_agent_runs: u32,
    pub resource_pools: Vec<ResourcePool>,
    pub mcp_resource_assignments: BTreeMap<String, String>,
    #[serde(default)]
    pub updated_at: Option<i64>,
}
impl Default for SchedulerConfiguration {
    fn default() -> Self {
        Self {
            revision: 1,
            max_active_runs_per_user: 2,
            max_global_agent_runs: 1000,
            resource_pools: vec![],
            mcp_resource_assignments: BTreeMap::new(),
            updated_at: None,
        }
    }
}
impl SchedulerConfiguration {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=128).contains(&self.max_active_runs_per_user)
            || !(1..=1024).contains(&self.max_global_agent_runs)
            || self.resource_pools.len() > 128
            || self.mcp_resource_assignments.len() > 1000
        {
            return Err("invalid scheduler limits");
        }
        let mut ids = BTreeSet::new();
        for pool in &self.resource_pools {
            if pool.id.is_empty()
                || pool.id.len() > 64
                || !pool
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                || pool.name.trim().is_empty()
                || pool.name.chars().count() > 100
                || pool.name.chars().any(char::is_control)
                || !(1..=1024).contains(&pool.max_concurrency)
                || !ids.insert(&pool.id)
            {
                return Err("invalid or duplicate resource pool");
            }
        }
        if self.mcp_resource_assignments.iter().any(|(id, pool)| {
            id.is_empty()
                || id.len() > 512
                || id.chars().any(char::is_control)
                || !ids.contains(pool)
        }) {
            return Err("assignment must reference an existing resource pool");
        }
        Ok(())
    }
}
