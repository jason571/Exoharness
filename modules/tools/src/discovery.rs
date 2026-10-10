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
    terms.into_iter().collect()
}
