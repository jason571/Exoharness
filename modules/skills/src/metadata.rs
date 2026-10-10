// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Mature frontmatter reader reused from the original standalone Skill loader.
use crate::{Result, errors::validation};
use std::collections::BTreeMap;

pub(crate) fn identity(content: &str) -> Result<(String, String)> {
    let values = parse_frontmatter(content.trim_start_matches('\u{feff}'))?;
    Ok((
        required_frontmatter(&values, "name")?.into(),
        required_frontmatter(&values, "description")?.into(),
    ))
}

fn parse_frontmatter(content: &str) -> Result<BTreeMap<String, String>> {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.first().copied() != Some("---") {
        return Err(validation("SKILL.md must start with YAML frontmatter"));
    }
    let mut values = BTreeMap::new();
    let mut closed = false;
    let mut index = 1;
    while index < lines.len() {
        let line = lines[index];
        if line == "---" {
            closed = true;
            break;
        }
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            index += 1;
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            // Nested values on optional metadata are preserved in SKILL.md but
            // are not interpreted by the registry. Required top-level fields
            // are validated after parsing.
            index += 1;
            continue;
        }
        if line.starts_with('-') {
            return Err(validation("top-level Skill frontmatter must be a mapping"));
        }
        let (key, raw_value) = line
            .split_once(':')
            .ok_or_else(|| validation("invalid Skill frontmatter field"))?;
        let key = key.trim();
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(validation("invalid Skill frontmatter key"));
        }
        let raw_value = raw_value.trim();
        let value = if matches!(raw_value, "|" | "|-" | "|+" | ">" | ">-" | ">+") {
            let folded = raw_value.starts_with('>');
            index += 1;
            let mut block = Vec::new();
            while index < lines.len() {
                let block_line = lines[index];
                if block_line == "---"
                    || (!block_line.trim().is_empty()
                        && !block_line.starts_with(char::is_whitespace))
                {
                    break;
                }
                block.push(block_line.trim_start());
                index += 1;
            }
            parse_block_scalar(&block, folded)?
        } else {
            index += 1;
            // Only identity fields are interpreted. Optional YAML metadata is
            // opaque and remains byte-for-byte in the immutable package.
            if matches!(key, "name" | "description") {
                parse_scalar(raw_value)?
            } else {
                raw_value.to_owned()
            }
        };
        if values.insert(key.to_owned(), value).is_some() {
            return Err(validation(format!(
                "duplicate Skill frontmatter field {key:?}"
            )));
        }
    }
    if !closed {
        return Err(validation("SKILL.md frontmatter has no closing delimiter"));
    }
    Ok(values)
}

fn parse_block_scalar(lines: &[&str], folded: bool) -> Result<String> {
    if lines.iter().any(|line| line.contains(['\0', '\r'])) {
        return Err(validation("invalid control character in Skill frontmatter"));
    }
    if !folded {
        return Ok(lines.join("\n").trim().to_owned());
    }
    let mut value = String::new();
    let mut paragraph_break = false;
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            paragraph_break = !value.is_empty();
            continue;
        }
        if !value.is_empty() {
            value.push(if paragraph_break { '\n' } else { ' ' });
        }
        value.push_str(line);
        paragraph_break = false;
    }
    Ok(value)
}

fn parse_scalar(value: &str) -> Result<String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.starts_with(['[', '{', '&', '*', '!', '|', '>']) {
        return Err(validation(
            "complex YAML values are not supported in Skill frontmatter",
        ));
    }
    let unquoted = if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    };
    if unquoted.contains(['\0', '\r', '\n']) {
        return Err(validation("invalid control character in Skill frontmatter"));
    }
    Ok(unquoted.to_owned())
}

fn required_frontmatter<'a>(values: &'a BTreeMap<String, String>, key: &str) -> Result<&'a str> {
    values
        .get(key)
        .map(String::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| validation(format!("Skill frontmatter requires {key:?}")))
}
