// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::CapabilityManifest;
use serde::{Deserialize, Serialize};

/// One consistent database projection, not independently cached lifecycle state.
#[derive(Debug, Clone)]
pub struct RegisteredCapabilityView {
    pub capability: StoredCapability,
    pub permissions: Vec<PermissionRecord>,
    pub health: Option<CapabilityHealth>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoredCapability {
    pub id: String,
    pub manifest: CapabilityManifest,
    pub manifest_digest: String,
    #[serde(default)]
    pub source_digest: Option<String>,
    pub revision: i64,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    Pending,
    Allow,
    Deny,
}

impl PermissionDecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Allow => "allow",
            Self::Deny => "deny",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "allow" => Some(Self::Allow),
            "deny" => Some(Self::Deny),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionRecord {
    pub capability_id: String,
    pub permission: String,
    pub decision: PermissionDecision,
    pub decided_by: Option<String>,
    pub decided_at: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Unknown,
    Healthy,
    Degraded,
    Unavailable,
}

impl HealthStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unavailable => "unavailable",
        }
    }

    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "unknown" => Some(Self::Unknown),
            "healthy" => Some(Self::Healthy),
            "degraded" => Some(Self::Degraded),
            "unavailable" => Some(Self::Unavailable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityHealth {
    pub capability_id: String,
    pub status: HealthStatus,
    pub message: Option<String>,
    pub checked_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityVersion {
    pub capability_id: String,
    pub revision: i64,
    pub version: String,
    pub manifest: CapabilityManifest,
    pub manifest_digest: String,
    pub source_digest: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub capability_id: Option<String>,
    pub details: serde_json::Value,
    pub created_at: i64,
}
