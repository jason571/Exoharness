// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
use crate::{Result, ensure};
use fs2::FileExt;
use serde_json::Value;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
};

pub fn no_links(path: &Path) -> Result<()> {
    let mut current = std::path::PathBuf::new();
    for part in path.components() {
        ensure(
            !matches!(part, Component::ParentDir),
            "Parent traversal is forbidden",
        )?;
        current.push(part);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            ensure(
                !metadata.file_type().is_symlink(),
                "Evaluation paths must not contain symlinks",
            )?;
        }
    }
    Ok(())
}
pub fn private_dir(path: &Path) -> Result<()> {
    no_links(path)?;
    if !path.exists() {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path)?;
    }
    ensure(path.is_dir(), "Expected a private directory")?;
    #[cfg(unix)]
    ensure(
        fs::metadata(path)?.permissions().mode() & 0o077 == 0,
        "Evaluation directory must be private",
    )?;
    Ok(())
}
pub fn read(path: &Path) -> Result<Value> {
    no_links(path)?;
    let meta = fs::metadata(path)?;
    ensure(
        meta.is_file() && meta.len() <= 32 * 1024 * 1024,
        "Evaluation input must be a bounded regular file",
    )?;
    Ok(serde_json::from_reader(File::open(path)?)?)
}
pub fn read_private(path: &Path) -> Result<Value> {
    #[cfg(unix)]
    ensure(
        fs::metadata(path)?.permissions().mode() & 0o077 == 0,
        "Credentials and evidence must be private",
    )?;
    read(path)
}
pub fn bytes(path: &Path, maximum: u64) -> Result<Vec<u8>> {
    no_links(path)?;
    ensure(fs::metadata(path)?.is_file(), "Expected a regular file")?;
    let mut value = Vec::new();
    File::open(path)?
        .take(maximum + 1)
        .read_to_end(&mut value)?;
    ensure(
        value.len() as u64 <= maximum,
        "File exceeds evaluation size limit",
    )?;
    Ok(value)
}
pub fn write(path: &Path, value: &Value) -> Result<()> {
    let mut raw = serde_json::to_vec_pretty(value)?;
    raw.push(b'\n');
    write_bytes(path, &raw)
}
pub fn write_bytes(path: &Path, raw: &[u8]) -> Result<()> {
    no_links(path)?;
    let parent = path
        .parent()
        .ok_or_else(|| crate::Error::Invalid("Missing evidence parent".into()))?;
    if !parent.exists() {
        private_dir(parent)?;
    }
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    temporary.write_all(raw)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub struct Exclusive(File);
impl Exclusive {
    pub fn acquire(root: &Path) -> Result<Self> {
        private_dir(root)?;
        let path = root.join(".lock");
        no_links(&path)?;
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(unix)]
        options.mode(0o600);
        let file = options.open(path)?;
        file.try_lock_exclusive()?;
        Ok(Self(file))
    }
}
impl Drop for Exclusive {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
