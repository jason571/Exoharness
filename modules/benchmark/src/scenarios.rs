// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Result, Suite, contracts::sha256, ensure, evidence};
use std::path::Path;
pub fn suite(kind: &str, audio_sha: Option<&str>) -> Result<Suite> {
    let data = match kind {
        "text-smoke" => include_str!("scenarios/text-smoke.json"),
        "workflows" => include_str!("scenarios/workflows.json"),
        "current-reports" => include_str!("scenarios/current-reports.json"),
        _ => return Err(crate::Error::Invalid("Unknown suite kind".into())),
    };
    let mut value: serde_json::Value = serde_json::from_str(data)?;
    if kind == "workflows" {
        let sha = audio_sha.ok_or_else(|| {
            crate::Error::Invalid("Workflow suite requires pinned WAV audio".into())
        })?;
        ensure(
            crate::contracts::hash(&serde_json::json!(sha)),
            "Invalid audio digest",
        )?;
        for task in value["tasks"].as_array_mut().expect("embedded suite") {
            if let Some(a) = task
                .get_mut("attachments")
                .and_then(serde_json::Value::as_array_mut)
            {
                for item in a {
                    item["sha256"] = serde_json::json!(sha);
                }
            }
        }
    }
    Suite::parse(value)
}
pub fn create(root: &Path, kind: &str, audio: Option<&Path>) -> Result<Suite> {
    evidence::no_links(root)?;
    ensure(
        !root.exists() || std::fs::read_dir(root)?.next().is_none(),
        "Create a suite in a new empty directory",
    )?;
    let data = if kind == "workflows" {
        let raw = evidence::bytes(
            audio.ok_or_else(|| crate::Error::Invalid("Existing WAV input required".into()))?,
            32 * 1024 * 1024,
        )?;
        validate_wav(&raw)?;
        Some(raw)
    } else {
        None
    };
    let sha = data.as_ref().map(|v| sha256(v));
    let suite = suite(kind, sha.as_deref())?;
    evidence::private_dir(root)?;
    if let Some(raw) = data {
        evidence::write_bytes(&root.join("inputs/existing.wav"), &raw)?;
    }
    evidence::write(&root.join("suite.json"), suite.value())?;
    Ok(suite)
}
pub fn validate_wav(raw: &[u8]) -> Result<()> {
    ensure(
        raw.len() >= 12 && &raw[..4] == b"RIFF" && &raw[8..12] == b"WAVE",
        "Expected PCM WAV",
    )?;
    let mut offset = 12;
    let (mut format, mut frames) = (false, false);
    while offset + 8 <= raw.len() {
        let size = u32::from_le_bytes(raw[offset + 4..offset + 8].try_into().expect("four bytes"))
            as usize;
        let begin = offset + 8;
        let end = begin
            .checked_add(size)
            .ok_or_else(|| crate::Error::Invalid("WAV chunk overflow".into()))?;
        ensure(end <= raw.len(), "Truncated WAV chunk")?;
        if &raw[offset..offset + 4] == b"fmt " {
            ensure(size >= 16, "Invalid WAV format")?;
            let channels =
                u16::from_le_bytes(raw[begin + 2..begin + 4].try_into().expect("two bytes"));
            let rate =
                u32::from_le_bytes(raw[begin + 4..begin + 8].try_into().expect("four bytes"));
            format = u16::from_le_bytes(raw[begin..begin + 2].try_into().expect("two bytes")) == 1
                && (1..=2).contains(&channels)
                && rate > 0;
        }
        if &raw[offset..offset + 4] == b"data" {
            frames = size > 0;
        }
        offset = end + (size % 2);
    }
    ensure(format && frames, "Provide a nonempty mono/stereo PCM WAV")
}
