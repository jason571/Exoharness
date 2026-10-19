// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Bounded lexical terms, including Chinese words without whitespace.
//! This is not semantic routing and never grants or selects a provider.
use std::collections::BTreeSet;

pub(crate) fn terms(query: &str) -> Vec<String> {
    let lower = query.to_lowercase();
    let mut terms = BTreeSet::new();
    for word in lower.split_whitespace() {
        terms.insert(word.to_owned());
        let chars: Vec<_> = word.chars().collect();
        for pair in chars.windows(2) {
            if pair.iter().all(|c| ('\u{4e00}'..='\u{9fff}').contains(c)) {
                terms.insert(pair.iter().collect());
            }
        }
    }
    // CJK text does not normally separate embedded technical identifiers with
    // spaces. Keep the legacy terms and recover those identifiers as metadata
    // search terms; this never activates tools or widens the catalog.
    for word in
        lower.split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '+')))
    {
        let word = word.trim_matches(['_', '-', '.']);
        if word.chars().any(|c| c.is_ascii_alphabetic()) {
            terms.insert(word.to_owned());
        }
    }
    terms.into_iter().collect()
}
