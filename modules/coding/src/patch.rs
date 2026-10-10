//! Pure, strict patch preparation. No filesystem writes or execution.
//! Codex-style markers; workspace adapter commits the entire candidate with CAS.
use crate::paths::WorkspacePath;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("patch rejected: {0}; no file changed")]
pub struct PatchError(pub &'static str);
#[derive(Debug)]
pub enum Change {
    Add {
        path: String,
        content: String,
    },
    Delete {
        path: String,
    },
    Update {
        path: String,
        destination: Option<String>,
        chunks: Vec<Chunk>,
    },
}
#[derive(Debug, Default)]
pub struct Chunk {
    pub context: Option<String>,
    pub old: Vec<String>,
    pub new: Vec<String>,
    pub eof: bool,
}
#[derive(Debug, Default)]
pub struct Candidate {
    pub writes: BTreeMap<String, String>,
    pub deletes: Vec<String>,
}
fn path(text: &str) -> Result<String, PatchError> {
    WorkspacePath::parse(text).map_err(|_| PatchError("invalid relative path"))?;
    if text.split('/').next() == Some("skill") {
        return Err(PatchError("Skill package is read-only"));
    }
    Ok(text.into())
}
pub fn parse(text: &str) -> Result<Vec<Change>, PatchError> {
    if text.len() > 4 * 1024 * 1024 {
        return Err(PatchError("patch too large"));
    }
    let lines: Vec<_> = text.lines().collect();
    if lines.first() != Some(&"*** Begin Patch") || lines.last() != Some(&"*** End Patch") {
        return Err(PatchError("missing patch markers"));
    }
    let mut changes = Vec::new();
    let mut i = 1;
    while i < lines.len() - 1 {
        let line = lines[i];
        i += 1;
        if let Some(name) = line.strip_prefix("*** Add File: ") {
            let path = path(name)?;
            let mut content = String::new();
            while i < lines.len() - 1 && !lines[i].starts_with("*** ") {
                content.push_str(
                    lines[i]
                        .strip_prefix('+')
                        .ok_or(PatchError("add lines must start with +"))?,
                );
                content.push('\n');
                i += 1;
            }
            changes.push(Change::Add { path, content });
        } else if let Some(name) = line.strip_prefix("*** Delete File: ") {
            changes.push(Change::Delete { path: path(name)? });
        } else if let Some(name) = line.strip_prefix("*** Update File: ") {
            let path = path(name)?;
            let destination = if i < lines.len() - 1 {
                if let Some(name) = lines[i].strip_prefix("*** Move to: ") {
                    i += 1;
                    Some(self::path(name)?)
                } else {
                    None
                }
            } else {
                None
            };
            let mut chunks = Vec::new();
            while i < lines.len() - 1 && !lines[i].starts_with("*** ") {
                let mut chunk = Chunk::default();
                if lines[i] == "@@" {
                    i += 1;
                } else if let Some(context) = lines[i].strip_prefix("@@ ") {
                    chunk.context = Some(context.into());
                    i += 1;
                } else if !chunks.is_empty() {
                    return Err(PatchError("invalid hunk header"));
                }
                while i < lines.len() - 1
                    && !lines[i].starts_with("@@")
                    && !lines[i].starts_with("*** ")
                {
                    let line = lines[i];
                    i += 1;
                    match line.as_bytes().first() {
                        Some(b' ') => {
                            chunk.old.push(line[1..].into());
                            chunk.new.push(line[1..].into());
                        }
                        Some(b'-') => chunk.old.push(line[1..].into()),
                        Some(b'+') => chunk.new.push(line[1..].into()),
                        _ => return Err(PatchError("invalid hunk line")),
                    }
                }
                if i < lines.len() - 1 && lines[i] == "*** End of File" {
                    chunk.eof = true;
                    i += 1;
                }
                if chunk.old.is_empty() {
                    return Err(PatchError(
                        "update needs existing context; use Add File for new files",
                    ));
                }
                chunks.push(chunk);
            }
            if chunks.is_empty() && destination.is_none() {
                return Err(PatchError("empty update"));
            }
            changes.push(Change::Update {
                path,
                destination,
                chunks,
            });
        } else {
            return Err(PatchError("unsupported patch directive"));
        }
        if changes.len() > 128 {
            return Err(PatchError("too many file changes"));
        }
    }
    if changes.is_empty() {
        return Err(PatchError("empty patch"));
    }
    Ok(changes)
}
pub fn affected(changes: &[Change]) -> BTreeSet<String> {
    changes
        .iter()
        .flat_map(|c| match c {
            Change::Add { path, .. } | Change::Delete { path } => vec![path.clone()],
            Change::Update {
                path, destination, ..
            } => std::iter::once(path.clone())
                .chain(destination.clone())
                .collect(),
        })
        .collect()
}
pub fn prepare(
    changes: &[Change],
    sources: &BTreeMap<String, String>,
) -> Result<Candidate, PatchError> {
    let mut candidate = Candidate::default();
    let mut touched = BTreeSet::new();
    for change in changes {
        let (name, destination) = match change {
            Change::Add { path, .. } | Change::Delete { path } => (path, None),
            Change::Update {
                path, destination, ..
            } => (path, destination.as_ref()),
        };
        if !touched.insert(name.clone())
            || destination.is_some_and(|d| d == name || !touched.insert(d.clone()))
        {
            return Err(PatchError("conflicting file operations"));
        }
        match change {
            Change::Add { path, content } => {
                if sources.contains_key(path) {
                    return Err(PatchError("Add File cannot overwrite an existing file"));
                }
                candidate.writes.insert(path.clone(), content.clone());
            }
            Change::Delete { path } => {
                if !sources.contains_key(path) {
                    return Err(PatchError("file does not exist"));
                }
                candidate.deletes.push(path.clone());
            }
            Change::Update {
                path,
                destination,
                chunks,
            } => {
                let source = sources.get(path).ok_or(PatchError("file does not exist"))?;
                if destination
                    .as_ref()
                    .is_some_and(|d| sources.contains_key(d))
                {
                    return Err(PatchError("move cannot overwrite a file"));
                }
                let mut lines: Vec<String> = source.lines().map(str::to_owned).collect();
                let mut ranges = Vec::new();
                let mut after = 0;
                for chunk in chunks {
                    let context_start = if let Some(context) = &chunk.context {
                        let matches: Vec<_> = lines
                            .iter()
                            .enumerate()
                            .skip(after)
                            .filter(|(_, line)| *line == context)
                            .take(2)
                            .collect();
                        if matches.len() != 1 {
                            return Err(PatchError("context is missing or ambiguous"));
                        }
                        matches[0].0 + 1
                    } else {
                        after
                    };
                    let matches: Vec<_> = lines
                        .windows(chunk.old.len())
                        .enumerate()
                        .skip(context_start)
                        .filter(|(i, window)| {
                            *window == chunk.old
                                && (!chunk.eof || i + chunk.old.len() == lines.len())
                        })
                        .take(2)
                        .map(|(i, _)| i)
                        .collect();
                    if matches.len() != 1 {
                        return Err(PatchError(
                            "hunk is missing or ambiguous; read focused context",
                        ));
                    }
                    let start = matches[0];
                    let end = start + chunk.old.len();
                    after = end;
                    ranges.push((start, end, &chunk.new));
                }
                for (start, end, replacement) in ranges.into_iter().rev() {
                    lines.splice(start..end, replacement.iter().cloned());
                }
                let separator = if source.contains("\r\n") {
                    "\r\n"
                } else {
                    "\n"
                };
                let mut text = lines.join(separator);
                if source.ends_with('\n') && !lines.is_empty() {
                    text.push_str(separator);
                }
                let target = destination.as_ref().unwrap_or(path).clone();
                candidate.writes.insert(target, text);
                if destination.is_some() {
                    candidate.deletes.push(path.clone());
                }
            }
        }
    }
    Ok(candidate)
}
