use crate::{
    Result,
    errors::validation,
    types::{MAX_FILE_BYTES, MAX_FILES, MAX_TOTAL_BYTES, PackageFiles},
};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
};

pub fn normalize_files(raw_files: Vec<(String, Vec<u8>)>) -> Result<PackageFiles> {
    if raw_files.is_empty() {
        return Err(validation("package must contain at least one file"));
    }
    if raw_files.len() > MAX_FILES {
        return Err(validation(format!(
            "package contains more than {MAX_FILES} files"
        )));
    }

    let mut split_paths = Vec::with_capacity(raw_files.len());
    let mut total = 0_usize;
    for (path, bytes) in &raw_files {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(validation(format!(
                "file {path:?} exceeds {MAX_FILE_BYTES} bytes"
            )));
        }
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| validation("package is too large"))?;
        if total > MAX_TOTAL_BYTES {
            return Err(validation(format!(
                "package exceeds {MAX_TOTAL_BYTES} bytes"
            )));
        }
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path.contains('\0')
            || path.split('/').any(str::is_empty)
            || (path.len() >= 2 && path.as_bytes()[1] == b':')
        {
            return Err(validation(format!("unsafe package path {path:?}")));
        }
        let components = path
            .split('/')
            .filter(|component| !component.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if components.is_empty()
            || components.len() > 64
            || components.iter().any(|component| {
                component == "."
                    || component == ".."
                    || component.contains(':')
                    || component.chars().any(char::is_control)
            })
        {
            return Err(validation(format!("unsafe package path {path:?}")));
        }
        split_paths.push(components);
    }

    let mut files = BTreeMap::new();
    let mut case_folded = HashSet::new();
    for ((_, bytes), components) in raw_files.into_iter().zip(split_paths) {
        let mut path = PathBuf::new();
        for component in &components {
            path.push(component);
        }
        let folded = path.to_string_lossy().to_ascii_lowercase();
        if !case_folded.insert(folded) || files.insert(path.clone(), bytes).is_some() {
            return Err(validation(format!(
                "package contains a duplicate or case-colliding path {path:?}"
            )));
        }
    }
    Ok(files)
}

/// Remove exactly one shared top-level wrapper directory.
///
/// Callers should only use this candidate after checking that the expected
/// package entrypoint is absent from the original layout and present here.
pub fn without_common_root(files: PackageFiles) -> std::result::Result<PackageFiles, PackageFiles> {
    let common_root = files
        .keys()
        .next()
        .and_then(|path| path.components().next())
        .map(|component| component.as_os_str().to_owned());
    let Some(common_root) = common_root else {
        return Err(files);
    };
    if !files.keys().all(|path| {
        let mut components = path.components();
        components
            .next()
            .is_some_and(|component| component.as_os_str() == common_root)
            && components.next().is_some()
    }) {
        return Err(files);
    }
    let mut stripped = BTreeMap::new();
    for (path, content) in files {
        let path = path.components().skip(1).collect::<PathBuf>();
        stripped.insert(path, content);
    }
    Ok(stripped)
}
