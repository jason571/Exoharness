use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// Model-visible projection is rebuilt from immutable journal data. Complete
/// arguments/results stay in the journal; this module never edits originals.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum JournalEntry {
    User(String),
    Assistant(String),
    ToolExchange {
        invocation_id: String,
        tool: String,
        arguments: Value,
        result: Value,
    },
    Checkpoint {
        invocation_id: String,
        reference: String,
        summary: Value,
    },
    SkillLoaded {
        revision_key: String,
        instructions: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ContextItem {
    User(String),
    Assistant(String),
    /// Call/result are projected atomically; adapters cannot keep just one half.
    ToolExchange {
        invocation_id: String,
        tool: String,
        arguments: Value,
        result: Value,
    },
    Checkpoint {
        invocation_id: String,
        reference: String,
        summary: Value,
    },
    Skill {
        revision_key: String,
        instructions: String,
    },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ProjectionError {
    #[error("duplicate invocation identity: {0}")]
    DuplicateInvocation(String),
    #[error("checkpoint has no preceding completed exchange: {0}")]
    OrphanCheckpoint(String),
    #[error("checkpoint reference must not be blank")]
    EmptyReference,
    #[error("same immutable Skill revision has conflicting content: {0}")]
    ConflictingSkill(String),
}

/// Only explicitly selected Skill revisions are loaded, once each. A durable
/// checkpoint replaces the entire old call/result pair at its original position.
/// Tool selection and token budgeting belong to separate dispatcher policies.
pub fn project(
    entries: &[JournalEntry],
    selected_skills: &BTreeSet<String>,
) -> Result<Vec<ContextItem>, ProjectionError> {
    let mut out = Vec::new();
    let mut invocations = BTreeMap::new();
    let mut skills = BTreeMap::new();
    for entry in entries {
        match entry {
            JournalEntry::User(text) => out.push(ContextItem::User(text.clone())),
            JournalEntry::Assistant(text) => out.push(ContextItem::Assistant(text.clone())),
            JournalEntry::ToolExchange {
                invocation_id,
                tool,
                arguments,
                result,
            } => {
                if invocations.insert(invocation_id, out.len()).is_some() {
                    return Err(ProjectionError::DuplicateInvocation(invocation_id.clone()));
                }
                out.push(ContextItem::ToolExchange {
                    invocation_id: invocation_id.clone(),
                    tool: tool.clone(),
                    arguments: arguments.clone(),
                    result: result.clone(),
                });
            }
            JournalEntry::Checkpoint {
                invocation_id,
                reference,
                summary,
            } => {
                if reference.trim().is_empty() {
                    return Err(ProjectionError::EmptyReference);
                }
                let index = invocations
                    .get(invocation_id)
                    .ok_or_else(|| ProjectionError::OrphanCheckpoint(invocation_id.clone()))?;
                out[*index] = ContextItem::Checkpoint {
                    invocation_id: invocation_id.clone(),
                    reference: reference.clone(),
                    summary: summary.clone(),
                };
            }
            JournalEntry::SkillLoaded {
                revision_key,
                instructions,
            } => {
                if let Some(previous) = skills.insert(revision_key, instructions) {
                    if previous != instructions {
                        return Err(ProjectionError::ConflictingSkill(revision_key.clone()));
                    }
                    continue;
                }
                if selected_skills.contains(revision_key) {
                    out.push(ContextItem::Skill {
                        revision_key: revision_key.clone(),
                        instructions: instructions.clone(),
                    });
                }
            }
        }
    }
    Ok(out)
}
