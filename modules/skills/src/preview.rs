use crate::{
    Inspection, Preview, Result,
    types::{FileChange, PackageFiles, PermissionChanges},
};
use exoharness_contracts::capability::StoredCapability;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(crate) fn build(
    proposed: &Inspection,
    current: Option<&StoredCapability>,
    existing: Option<&PackageFiles>,
) -> Result<Preview> {
    let manifest_changed = current.is_some_and(|value| value.manifest != proposed.manifest);
    let content_changed = current
        .is_some_and(|value| value.source_digest.as_deref() != Some(&proposed.source_digest));
    let action = match current {
        None => "create",
        Some(_) if manifest_changed || content_changed => "update",
        Some(_) => "unchanged",
    };
    let previous = current
        .map(|c| {
            c.manifest
                .permissions
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let next = proposed
        .manifest
        .permissions
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let empty = PackageFiles::new();
    let existing = existing.unwrap_or(&empty);
    let paths = existing
        .keys()
        .chain(proposed.content.keys())
        .collect::<BTreeSet<_>>();
    let file_changes = paths
        .into_iter()
        .filter_map(|path| {
            let old = existing.get(path);
            let new = proposed.content.get(path);
            let change = match (old, new) {
                (None, Some(_)) => "added",
                (Some(_), None) => "removed",
                (Some(a), Some(b)) if Sha256::digest(a) != Sha256::digest(b) => "modified",
                _ => return None,
            };
            Some(FileChange {
                path: path.clone(),
                change,
                previous_size: old.map(Vec::len),
                proposed_size: new.map(Vec::len),
            })
        })
        .collect();
    Ok(Preview {
        action,
        requires_confirmation: action == "update",
        capability_id: proposed.manifest.id(),
        name: proposed.manifest.name.clone(),
        current_revision: current.map(|c| c.revision),
        current_version: current.map(|c| c.manifest.version.clone()),
        proposed_version: proposed.manifest.version.clone(),
        current_digest: current.and_then(|c| c.source_digest.clone()),
        proposed_digest: proposed.source_digest.clone(),
        enabled: current.map(|c| c.enabled),
        manifest_changed,
        permission_changes: PermissionChanges {
            added: next.difference(&previous).cloned().collect(),
            removed: previous.difference(&next).cloned().collect(),
        },
        file_changes,
    })
}
