// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bounded literal search over caller-authorized UTF-8 documents.
#[derive(Debug, Clone)]
pub struct Match {
    pub path: String,
    pub line: usize,
    pub text: String,
    pub truncated: bool,
}
pub fn matches<'a>(
    path: &'a str,
    text: &'a str,
    query: &'a str,
    limit: usize,
) -> impl Iterator<Item = Match> + 'a {
    text.lines()
        .enumerate()
        .filter(move |(_, line)| !query.is_empty() && line.contains(query))
        .take(limit)
        .map(move |(i, line)| Match {
            path: path.into(),
            line: i + 1,
            text: line.chars().take(300).collect(),
            truncated: line.chars().count() > 300,
        })
}
