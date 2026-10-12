// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Preserve each distinct source/range; replace only a covered duplicate body.
use exoharness_core::context::ContextItem;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) struct ReadBody<'a> {
    pub workspace: &'a str,
    pub path: &'a str,
    pub hash: &'a str,
    pub revision: u64,
    pub start: usize,
    pub end: usize,
    pub content: &'a str,
    pub terminal: bool,
}
impl<'a> ReadBody<'a> {
    pub fn parse(value: &'a Value) -> Option<Self> {
        if value["_durable_read_body"] != true || value["isError"] == true {
            return None;
        }
        let hash = value["file"]["sha256"].as_str()?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let content = value["content"].as_str()?;
        let start = usize::try_from(value["offset"].as_u64()?).ok()?;
        Some(Self {
            workspace: value["workspace_id"].as_str()?,
            path: value["file"]["path"].as_str()?,
            hash,
            revision: value["revision"].as_u64()?,
            start,
            end: start.checked_add(content.chars().count())?,
            content,
            terminal: value.get("next_offset").is_some_and(Value::is_null),
        })
    }
}
struct Retained {
    start: usize,
    end: usize,
    chars: Vec<char>,
    reference: String,
}
pub(super) fn compact_previous_reads(items: &mut [ContextItem]) {
    let mut kept = BTreeMap::<(String, String, String), Vec<Retained>>::new();
    for item in items.iter_mut().rev() {
        let ContextItem::Checkpoint {
            summary, reference, ..
        } = item
        else {
            continue;
        };
        let Some(read) = ReadBody::parse(summary) else {
            continue;
        };
        let key = (
            read.workspace.to_owned(),
            read.path.to_owned(),
            read.hash.to_owned(),
        );
        let ranges = kept.entry(key).or_default();
        let covered = ranges.iter().find(|r| {
            r.start <= read.start
                && r.end >= read.end
                && r.chars[read.start - r.start..read.end - r.start]
                    .iter()
                    .copied()
                    .eq(read.content.chars())
        });
        if let Some(covered) = covered {
            let count = read.end - read.start;
            let target = covered.reference.clone();
            let object = summary.as_object_mut().expect("verified object receipt");
            object.remove("content");
            summary["read_body_retained_in_journal"] = json!(true);
            summary["read_body_chars"] = json!(count);
            summary["read_body_reference"] = json!(target);
        } else {
            ranges.push(Retained {
                start: read.start,
                end: read.end,
                chars: read.content.chars().collect(),
                reference: reference.clone(),
            });
        }
    }
}
