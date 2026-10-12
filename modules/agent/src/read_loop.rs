// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Semantic read review from host-verified immutable receipts, never MCP hints.
use crate::checkpoint_reads::ReadBody;
use exoharness_contracts::model::ToolCall;
use exoharness_core::context::JournalEntry;
use serde_json::{Value, json};

pub(super) fn repeated(entries: &[JournalEntry], call: &ToolCall) -> Option<Value> {
    let workspace = call.arguments["workspace_id"].as_str()?;
    let path = call.arguments["path"].as_str()?;
    let offset = usize::try_from(
        call.arguments
            .get("offset")
            .map_or(Some(0), Value::as_u64)?,
    )
    .ok()?;
    let limit = usize::try_from(
        call.arguments
            .get("limit")
            .map_or(Some(8000), Value::as_u64)?,
    )
    .ok()?;
    if !(1..=16000).contains(&limit) {
        return None;
    }
    // A failed OCI process can change source before its next verified read.
    // Do not infer current identity from a pre-failure checkpoint.
    for e in entries.iter().rev() {
        match e {
            JournalEntry::Checkpoint { summary, .. } if summary["workspace_id"] == workspace => {
                break;
            }
            JournalEntry::ToolExchange { result, .. }
                if result["_host_sandbox_failure"] == 1 && result["workspace_id"] == workspace =>
            {
                return None;
            }
            _ => {}
        }
    }
    let revision = entries.iter().rev().find_map(|e| match e {
        JournalEntry::Checkpoint { summary, .. } if summary["workspace_id"] == workspace => {
            summary["revision"].as_u64()
        }
        _ => None,
    })?;
    // Workspace revisions also advance for unrelated edits and finished checks.
    // Confirm this particular file's latest hash instead of resetting by revision.
    let hash = entries.iter().rev().find_map(|e| match e {
        JournalEntry::Checkpoint { summary, .. }
            if summary["workspace_id"] == workspace && summary["revision"] == revision =>
        {
            if summary["file"]["path"] == path {
                Some(summary["file"]["sha256"].as_str())
            } else if let Some(files) = summary["files"].as_array() {
                match files.iter().find(|f| f["path"] == path) {
                    Some(file) => Some(file["sha256"].as_str()),
                    None if summary["file_inventory_truncated"] != true => Some(None),
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    })??;
    let begin = entries
        .iter()
        .rposition(|e| matches!(e, JournalEntry::User(_)))
        .map_or(0, |i| i + 1);
    let reads: Vec<_> = entries[begin..]
        .iter()
        .filter_map(|e| {
            let JournalEntry::Checkpoint {
                invocation_id,
                reference,
                summary,
            } = e
            else {
                return None;
            };
            if reference.trim().is_empty() {
                return None;
            }
            let read = ReadBody::parse(summary)?;
            if read.workspace != workspace
                || read.path != path
                || read.hash != hash
                || read.revision > revision
            {
                return None;
            }
            let tool = entries.iter().find_map(|exchange| match exchange {
                JournalEntry::ToolExchange {
                    invocation_id: id,
                    tool,
                    ..
                } if id == invocation_id => Some(tool),
                _ => None,
            })?;
            (tool == &call.name).then_some((invocation_id, reference, read))
        })
        .collect();
    reads.last()?;
    let known = crate::repair_context::known_text(entries, workspace, path, hash);
    let eof = known.as_ref().map(|text| text.chars().count()).or_else(|| {
        reads
            .iter()
            .rev()
            .find(|(_, _, r)| r.hash == hash && r.terminal)
            .map(|(_, _, r)| r.end)
    });
    let start = match call.arguments.get("query") {
        None => offset.min(eof.unwrap_or(usize::MAX)),
        Some(value) => {
            let query = value.as_str()?;
            if query.is_empty() {
                return None;
            }
            let tail = if let Some(text) = known.as_ref() {
                text.chars().skip(offset).collect::<String>()
            } else {
                let (_, _, source) = reads.iter().rev().find(|(_, _, r)| {
                    r.hash == hash && r.terminal && r.start <= offset && r.end >= offset
                })?;
                source
                    .content
                    .chars()
                    .skip(offset - source.start)
                    .collect::<String>()
            };
            offset.checked_add(tail[..tail.find(query)?].chars().count())?
        }
    };
    let end = start.checked_add(limit)?.min(eof.unwrap_or(usize::MAX));
    if end < start {
        return None;
    }
    let mut ids = std::collections::BTreeSet::new();
    let receipts: Vec<_> = reads
        .iter()
        .filter(|(id, _, r)| {
            r.hash == hash && r.start == start && r.end >= end && ids.insert(id.as_str())
        })
        .collect();
    if receipts.len() < 2
        || receipts.iter().any(|(_, _, r)| {
            r.content.chars().take(end - start).ne(receipts
                .last()
                .expect("nonempty")
                .2
                .content
                .chars()
                .take(end - start))
        })
    {
        return None;
    }
    Some(
        json!({"tool":call.name,"workspace_id":workspace,"path":path,"sha256":hash,"revision":revision,
        "offset":start,"length":end-start,"receipt_references":receipts.iter().map(|(_,reference,_)|*reference).collect::<Vec<_>>()}),
    )
}
pub(super) const GUIDANCE: &str = "Host read-loop review: A proposed read was not submitted because the requested excerpt at this source version and starting position is already covered by two completed reads, including non-consecutive reads. Do not shrink the limit or vary the query to fetch the same known excerpt again. Canonical source bodies remain in the current context; use the existing receipt references. Diagnose the actual check or parameter failure, then make a focused authorized correction or package and publish existing work. Do not cycle through unchanged files or rerun completed generation. Do not infer new user requirements from a test's naming assumptions alone: compare failed checks to the actual source, preserve explicit user requirements, live Schemas and Skill contracts, and correct unsupported assumptions. This review cannot grant capabilities or bypass execution permissions.";

pub(super) fn context_reviews(entries: &[JournalEntry], reviews: &[Value]) -> Vec<Value> {
    reviews
        .iter()
        .map(|review| {
            let mut result = review.clone();
            let Some(workspace) = review["workspace_id"].as_str() else {
                return result;
            };
            let Some(path) = review["path"].as_str() else {
                return result;
            };
            let Some(hash) = review["sha256"].as_str() else {
                return result;
            };
            let Some(start) = review["offset"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
            else {
                return result;
            };
            let Some(length) = review["length"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .filter(|n| *n <= 16000)
            else {
                return result;
            };
            let call=ToolCall {id:"read-review-context".into(),name:review["tool"].as_str().unwrap_or("").into(),arguments:json!({"workspace_id":workspace,"path":path,"offset":start,"limit":length})};
            if repeated(entries,&call).is_none_or(|current|current["sha256"]!=hash) {return result}
            if let Some(text) = crate::repair_context::known_text(entries, workspace, path, hash) {
                result["requested_excerpt"] =
                    json!(text.chars().skip(start).take(length).collect::<String>());
            }
            result
        })
        .collect()
}
