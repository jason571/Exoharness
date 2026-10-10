use super::{TaskEdit, TaskSnapshot, TaskStore, files, revision_path};
use crate::{Result, SandboxError, filesystem};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
};
impl TaskStore {
    pub fn edit(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        revision: u64,
        operation: &str,
        edit: TaskEdit,
    ) -> Result<TaskSnapshot> {
        filesystem::identity(operation)?;
        let directory = self.directory(id)?;
        let _lock = self.lock(&directory)?;
        let mut state = self.reconcile(&directory, owner, session)?;
        if state.operations.contains_key(operation) {
            return Ok(state.snapshot);
        }
        if state.pending.is_some() {
            return Err(SandboxError::WorkspaceBusy);
        }
        if state.snapshot.revision != revision {
            return Err(SandboxError::StaleRevision);
        }
        if edit.files.len() + edit.patches.len() + edit.deletes.len() > 128
            || edit.notes.as_ref().is_some_and(|n| n.len() > 8000)
        {
            return Err(SandboxError::Invalid);
        }
        if edit.files.is_empty()
            && edit.patches.is_empty()
            && edit.deletes.is_empty()
            && edit
                .notes
                .as_ref()
                .is_none_or(|notes| notes == &state.snapshot.notes)
        {
            return Ok(state.snapshot);
        }
        let mut writes = BTreeSet::new();
        for path in edit.files.keys().chain(edit.deletes.iter()) {
            filesystem::relative(path)?;
            if path.split('/').next() == Some("skill") || !writes.insert(path.clone()) {
                return Err(SandboxError::Invalid);
            }
        }
        if edit.files.values().any(|s| s.len() > 4 * 1024 * 1024) {
            return Err(SandboxError::Invalid);
        }
        let source = revision_path(&directory, revision);
        let mut patches: BTreeMap<String, Vec<_>> = BTreeMap::new();
        for patch in edit.patches {
            filesystem::relative(&patch.path)?;
            if patch.path.split('/').next() == Some("skill") || writes.contains(&patch.path) {
                return Err(SandboxError::Invalid);
            }
            patches.entry(patch.path.clone()).or_default().push(patch);
        }
        let mut updates = edit.files;
        for (path, patches) in patches {
            let item = state
                .snapshot
                .files
                .iter()
                .find(|f| f.path == path)
                .ok_or(SandboxError::Invalid)?;
            if item.size_bytes > 4 * 1024 * 1024 {
                return Err(SandboxError::Invalid);
            }
            let text = fs::read_to_string(source.join(&path)).map_err(|_| SandboxError::Invalid)?;
            let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
            let mut ranges = Vec::new();
            for patch in patches {
                if patch.sha256 != hash {
                    return Err(SandboxError::StaleRevision);
                }
                if patch.before.is_empty() {
                    return Err(SandboxError::Invalid);
                }
                let matches = text
                    .char_indices()
                    .filter_map(|(i, _)| text[i..].starts_with(&patch.before).then_some(i))
                    .take(2)
                    .collect::<Vec<_>>();
                if matches.len() != 1 {
                    return Err(SandboxError::AmbiguousPatch);
                }
                let start = matches[0];
                ranges.push((start, start + patch.before.len(), patch.after));
            }
            ranges.sort_by_key(|r| r.0);
            if ranges.windows(2).any(|w| w[0].1 > w[1].0) {
                return Err(SandboxError::AmbiguousPatch);
            }
            let mut changed = text;
            for (start, end, after) in ranges.into_iter().rev() {
                changed.replace_range(start..end, &after);
            }
            updates.insert(path, changed);
        }
        let staged = files::stage(&directory)?;
        files::copy(&source, staged.path())?;
        for (path, text) in updates {
            files::write(staged.path(), &path, text.as_bytes())?;
        }
        for path in edit.deletes {
            fs::remove_file(staged.path().join(path)).map_err(|_| SandboxError::Invalid)?;
        }
        if let Some(notes) = edit.notes {
            state.snapshot.notes = notes;
        }
        self.install(&directory, &mut state, staged.path(), operation)?;
        Ok(state.snapshot)
    }
    /// Reads never return base64 or load unbounded files into model context.
    #[allow(clippy::too_many_arguments)]
    pub fn read(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        path: &str,
        offset: usize,
        limit: usize,
        query: Option<&str>,
    ) -> Result<serde_json::Value> {
        if limit == 0 || limit > 16000 {
            return Err(SandboxError::Invalid);
        }
        filesystem::relative(path)?;
        let directory = self.directory(id)?;
        let _lock = self.lock(&directory)?;
        let state = self.reconcile(&directory, owner, session)?;
        let file = state
            .snapshot
            .files
            .iter()
            .find(|f| f.path == path)
            .ok_or(SandboxError::Invalid)?;
        if file.size_bytes > 4 * 1024 * 1024 {
            return Err(SandboxError::Invalid);
        }
        let text =
            fs::read_to_string(revision_path(&directory, state.snapshot.revision).join(path))
                .map_err(|_| SandboxError::Invalid)?;
        if text.contains('\0') {
            return Err(SandboxError::Invalid);
        }
        let chars = text.chars().collect::<Vec<_>>();
        let start = if let Some(query) = query {
            if query.is_empty() || query.len() > 1000 {
                return Err(SandboxError::Invalid);
            }
            let skipped = chars.iter().take(offset).collect::<String>();
            text.get(skipped.len()..)
                .and_then(|tail| tail.find(query))
                .map(|index| offset + text[skipped.len()..skipped.len() + index].chars().count())
        } else {
            Some(offset.min(chars.len()))
        };
        let (content, next) = match start {
            Some(start) => {
                let end = (start + limit).min(chars.len());
                (
                    chars[start..end].iter().collect::<String>(),
                    (end < chars.len()).then_some(end),
                )
            }
            None => (String::new(), None),
        };
        Ok(
            serde_json::json!({"workspace_id":id,"revision":state.snapshot.revision,"file":file,"offset":start,"next_offset":next,"content":content}),
        )
    }
}
