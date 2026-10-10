// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

/// Snapshot of one authored UTF-8 file. Read and write belong to sandbox/storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub revision: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactEdit {
    pub before: String,
    pub after: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum EditError {
    #[error("saved file changed; read its current revision before editing")]
    StaleRevision,
    #[error("edit anchor must have exactly one match")]
    AmbiguousAnchor,
    #[error("file revision exhausted")]
    RevisionOverflow,
}

/// Pure all-or-nothing candidate generation. The caller must persist with CAS;
/// applying edits here alone does not authorize or submit a filesystem write.
pub fn apply_edits(
    document: &Document,
    expected_revision: u64,
    edits: &[ExactEdit],
) -> Result<Document, EditError> {
    if document.revision != expected_revision {
        return Err(EditError::StaleRevision);
    }
    let mut text = document.text.clone();
    for edit in edits {
        if edit.before.is_empty() {
            return Err(EditError::AmbiguousAnchor);
        }
        // Check every UTF-8 boundary, including overlapping occurrences.
        let matches: Vec<_> = text
            .char_indices()
            .filter_map(|(i, _)| text[i..].starts_with(&edit.before).then_some(i))
            .take(2)
            .collect();
        if matches.len() != 1 {
            return Err(EditError::AmbiguousAnchor);
        }
        let start = matches[0];
        text.replace_range(start..start + edit.before.len(), &edit.after);
    }
    if text == document.text {
        return Ok(document.clone());
    }
    let revision = document
        .revision
        .checked_add(1)
        .ok_or(EditError::RevisionOverflow)?;
    Ok(Document { revision, text })
}
