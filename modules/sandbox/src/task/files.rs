// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use super::TaskFile;
use crate::{Result, SandboxError, filesystem};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
const MAX_FILE: u64 = 128 * 1024 * 1024;
const MAX_TOTAL: u64 = 512 * 1024 * 1024;
pub(super) fn stage(root: &Path) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    builder.tempdir_in(root).map_err(|_| SandboxError::Storage)
}
pub(super) fn inventory(root: &Path) -> Result<Vec<TaskFile>> {
    filesystem::private(root)?;
    let mut result = Vec::new();
    walk(root, root, &mut result, 0)?;
    result.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}
fn walk(root: &Path, directory: &Path, result: &mut Vec<TaskFile>, depth: usize) -> Result<()> {
    if depth > 32 {
        return Err(SandboxError::Invalid);
    }
    for entry in fs::read_dir(directory).map_err(|_| SandboxError::Storage)? {
        let entry = entry.map_err(|_| SandboxError::Storage)?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|_| SandboxError::Storage)?;
        if meta.file_type().is_symlink() {
            return Err(SandboxError::Invalid);
        }
        if meta.is_dir() {
            walk(root, &path, result, depth + 1)?;
            continue;
        }
        if !meta.is_file() || meta.len() > MAX_FILE || result.len() >= 1024 {
            return Err(SandboxError::Invalid);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.nlink() != 1 {
                return Err(SandboxError::Invalid);
            }
        }
        let relative = path
            .strip_prefix(root)
            .map_err(|_| SandboxError::Invalid)?
            .to_str()
            .ok_or(SandboxError::Invalid)?;
        filesystem::relative(relative)?;
        let mut file = fs::File::open(&path).map_err(|_| SandboxError::Storage)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        let mut size = 0u64;
        loop {
            let n = file.read(&mut buffer).map_err(|_| SandboxError::Storage)?;
            if n == 0 {
                break;
            }
            size += n as u64;
            if size > MAX_FILE {
                return Err(SandboxError::Invalid);
            }
            hasher.update(&buffer[..n]);
        }
        if result.iter().map(|f| f.size_bytes).sum::<u64>() + size > MAX_TOTAL {
            return Err(SandboxError::Invalid);
        }
        result.push(TaskFile {
            path: relative.into(),
            size_bytes: size,
            sha256: format!("{:x}", hasher.finalize()),
        });
    }
    Ok(())
}
pub(super) fn copy(source: &Path, target: &Path) -> Result<()> {
    for file in inventory(source)? {
        if file.path.split('/').next() == Some("skill") {
            continue;
        }
        write_parent(target, &file.path)?;
        fs::copy(source.join(&file.path), target.join(&file.path))
            .map_err(|_| SandboxError::Storage)?;
        private_mode(&target.join(&file.path))?;
        fs::File::open(target.join(&file.path))
            .and_then(|f| f.sync_all())
            .map_err(|_| SandboxError::Storage)?;
    }
    fs::File::open(target)
        .and_then(|f| f.sync_all())
        .map_err(|_| SandboxError::Storage)?;
    Ok(())
}
fn private_mode(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|_| SandboxError::Storage)?;
    }
    Ok(())
}
pub(super) fn write_parent(root: &Path, relative: &str) -> Result<()> {
    let path = filesystem::relative(relative)?;
    let mut directory = root.to_path_buf();
    for part in path.parent().unwrap_or(Path::new("")).components() {
        directory.push(part);
        filesystem::private(&directory)?;
    }
    Ok(())
}
pub(super) fn write(root: &Path, relative: &str, bytes: &[u8]) -> Result<()> {
    write_parent(root, relative)?;
    let path = root.join(relative);
    if path.exists()
        && !fs::symlink_metadata(&path)
            .map_err(|_| SandboxError::Storage)?
            .is_file()
    {
        return Err(SandboxError::Invalid);
    }
    fs::write(&path, bytes).map_err(|_| SandboxError::Storage)?;
    private_mode(&path)?;
    fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|_| SandboxError::Storage)
}
