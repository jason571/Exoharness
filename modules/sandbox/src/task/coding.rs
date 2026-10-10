//! Filesystem adapter for pure Coding policy. All reads are scoped and locked;
//! patch candidates commit through the existing revisioned TaskStore.
use super::{TaskEdit, TaskSnapshot, TaskStore, revision_path};
use crate::{Result, SandboxError};
use exoharness_coding::{patch, search};
use serde_json::{Value, json};
use std::{collections::BTreeMap, fs};

impl TaskStore {
    #[allow(clippy::too_many_arguments)]
    pub fn apply_patch(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        revision: u64,
        operation: &str,
        text: &str,
    ) -> Result<TaskSnapshot> {
        let changes =
            patch::parse(text).map_err(|error| SandboxError::PatchRejected(error.to_string()))?;
        let candidate = {
            let directory = self.directory(id)?;
            let _lease = self.lock(&directory)?;
            let state = self.reconcile(&directory, owner, session)?;
            if state.operations.contains_key(operation) {
                return Ok(state.snapshot);
            }
            if state.pending.is_some() {
                return Err(SandboxError::WorkspaceBusy);
            }
            if state.snapshot.revision != revision {
                return Err(SandboxError::StaleRevision);
            }
            let mut sources = BTreeMap::new();
            let mut bytes = 0;
            for path in patch::affected(&changes) {
                if let Some(file) = state.snapshot.files.iter().find(|file| file.path == path) {
                    bytes += file.size_bytes;
                    if file.size_bytes > 4 * 1024 * 1024 || bytes > 16 * 1024 * 1024 {
                        return Err(SandboxError::Invalid);
                    }
                    let source =
                        fs::read_to_string(revision_path(&directory, revision).join(&path))
                            .map_err(|_| SandboxError::Invalid)?;
                    if source.contains('\0') {
                        return Err(SandboxError::Invalid);
                    }
                    sources.insert(path, source);
                }
            }
            patch::prepare(&changes, &sources)
                .map_err(|error| SandboxError::PatchRejected(error.to_string()))?
        };
        self.edit(
            owner,
            session,
            id,
            revision,
            operation,
            TaskEdit {
                files: candidate.writes,
                deletes: candidate.deletes,
                ..Default::default()
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search(
        &self,
        owner: &str,
        session: &str,
        id: &str,
        query: &str,
        prefix: &str,
        file_offset: usize,
        line_offset: usize,
        limit: usize,
    ) -> Result<Value> {
        if query.is_empty() || query.len() > 1000 || limit == 0 || limit > 100 {
            return Err(SandboxError::Invalid);
        }
        if !prefix.is_empty() {
            exoharness_coding::paths::WorkspacePath::parse(prefix)
                .map_err(|_| SandboxError::Invalid)?;
        }
        let directory = self.directory(id)?;
        let _lease = self.lock(&directory)?;
        let state = self.reconcile(&directory, owner, session)?;
        let mut matches = Vec::new();
        let mut skipped = Vec::new();
        let mut scanned_bytes = 0;
        let mut next = None;
        for (index, file) in state.snapshot.files.iter().enumerate().skip(file_offset) {
            if index >= file_offset.saturating_add(64) {
                next = Some(json!({"file_offset":index,"line_offset":0}));
                break;
            }
            if !prefix.is_empty()
                && file.path != prefix
                && !file.path.starts_with(&format!("{prefix}/"))
            {
                continue;
            }
            if file.size_bytes > 1024 * 1024 {
                skipped.push(file.path.clone());
                continue;
            }
            if scanned_bytes + file.size_bytes > 2 * 1024 * 1024 {
                next = Some(json!({"file_offset":index,"line_offset":0}));
                break;
            }
            scanned_bytes += file.size_bytes;
            let source =
                fs::read(revision_path(&directory, state.snapshot.revision).join(&file.path))
                    .map_err(|_| SandboxError::Storage)?;
            let Ok(text) = std::str::from_utf8(&source) else {
                skipped.push(file.path.clone());
                continue;
            };
            if text.contains('\0') {
                skipped.push(file.path.clone());
                continue;
            }
            for item in search::matches(&file.path, text, query, usize::MAX)
                .filter(|item| index != file_offset || item.line > line_offset)
            {
                matches.push(json!({"path":item.path,"line":item.line,"text":item.text,"truncated":item.truncated,"sha256":file.sha256}));
                if matches.len() == limit {
                    next = Some(json!({"file_offset":index,"line_offset":item.line}));
                    break;
                }
            }
            if next.is_some() {
                break;
            }
        }
        Ok(
            json!({"workspace_id":id,"revision":state.snapshot.revision,"matches":matches,"skipped_files":skipped,"next_cursor":next,"scanned_bytes":scanned_bytes,"complete":next.is_none()}),
        )
    }
}
