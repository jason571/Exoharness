use crate::invocation::InvocationState;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Running,
    AwaitingApproval,
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

impl RunState {
    pub fn active(self) -> bool {
        matches!(self, Self::Queued | Self::Running | Self::AwaitingApproval)
    }
    pub fn permits(self, next: Self) -> bool {
        match self {
            Self::Queued => matches!(next, Self::Running | Self::Cancelled | Self::Failed),
            Self::Running => matches!(
                next,
                Self::AwaitingApproval
                    | Self::Paused
                    | Self::Succeeded
                    | Self::Failed
                    | Self::Cancelled
            ),
            Self::AwaitingApproval => matches!(
                next,
                Self::Running | Self::Paused | Self::Failed | Self::Cancelled
            ),
            Self::Paused => matches!(next, Self::Running | Self::Failed | Self::Cancelled),
            Self::Succeeded | Self::Failed | Self::Cancelled => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    Resume,
    ChangeInput,
    ReconcileExternal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecoveryDecision {
    pub action: RecoveryAction,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelSnapshot {
    pub config_id: String,
    pub revision: u64,
    pub adapter: String,
    /// Includes nonsecret effective settings; credentials stay in Secret Store.
    pub settings: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub id: String,
    pub revision: u64,
    pub grant_revision: u64,
    pub allowed: bool,
    pub tool_schemas: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    pub version: u32,
    pub assistant_id: String,
    pub assistant_revision: u64,
    pub model: ModelSnapshot,
    pub capabilities: Vec<CapabilitySnapshot>,
    pub max_consecutive_identical_calls: usize,
    pub requires_execution: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: String,
    pub session_id: String,
    pub turn_id: String,
    pub state: RunState,
    pub version: u64,
    pub snapshot: ExecutionSnapshot,
    pub recovery: Option<RecoveryDecision>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub owner: String,
    pub title: String,
    pub updated_at: i64,
    pub version: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvocationIntent {
    /// Host-assigned identity, NOT the provider's transient tool_call_id.
    pub id: String,
    pub logical_key: String,
    pub capability_id: String,
    pub capability_revision: u64,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvocationRecord {
    pub run_id: String,
    pub intent: InvocationIntent,
    pub state: InvocationState,
    pub attempts: u64,
    pub result: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ExecutionEvent {
    /// Persisted UI projection. The journal remains the authoritative stream.
    Chat {
        payload: Value,
    },
    SessionCreated,
    TurnCreated {
        turn_id: String,
        run_id: String,
        input: String,
    },
    RunCreated {
        run_id: String,
        snapshot_digest: String,
    },
    RunTransition {
        run_id: String,
        from: RunState,
        to: RunState,
        recovery: Option<RecoveryDecision>,
    },
    InvocationPrepared {
        run_id: String,
        invocation_id: String,
        fingerprint: String,
    },
    InvocationSubmitted {
        run_id: String,
        invocation_id: String,
        attempt: u64,
    },
    InvocationResolved {
        run_id: String,
        invocation_id: String,
        state: InvocationState,
    },
    RecoveryUpdated {
        run_id: String,
        recovery: RecoveryDecision,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u32,
    pub session_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub event: ExecutionEvent,
}
