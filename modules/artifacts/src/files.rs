// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{ArtifactError, Result};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};
pub(crate) const MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024;
pub(crate) fn private(path: &Path) -> Result<()> {
    if !path.is_absolute() {
        return Err(ArtifactError::Storage);
    }
    if !path.try_exists().map_err(|_| ArtifactError::Storage)? {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path).map_err(|_| ArtifactError::Storage)?;
    }
    let meta = fs::symlink_metadata(path).map_err(|_| ArtifactError::Storage)?;
    if !meta.is_dir()
        || meta.file_type().is_symlink()
        || fs::canonicalize(path).map_err(|_| ArtifactError::Storage)? != path
    {
        return Err(ArtifactError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(ArtifactError::Storage);
        }
    }
    Ok(())
}
pub(crate) fn save(root: &Path, mut source: fs::File) -> Result<(String, u64)> {
    private(root)?;
    let meta = source.metadata().map_err(|_| ArtifactError::Storage)?;
    if !meta.is_file() || meta.len() > MAX_BYTES {
        return Err(ArtifactError::Invalid);
    }
    source.rewind().map_err(|_| ArtifactError::Storage)?;
    let parent = root.join("objects");
    private(&parent)?;
    let mut staged =
        tempfile::NamedTempFile::new_in(&parent).map_err(|_| ArtifactError::Storage)?;
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = source
            .read(&mut buffer)
            .map_err(|_| ArtifactError::Storage)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > MAX_BYTES {
            return Err(ArtifactError::Invalid);
        }
        hash.update(&buffer[..read]);
        staged
            .write_all(&buffer[..read])
            .map_err(|_| ArtifactError::Storage)?;
    }
    if total != meta.len() {
        return Err(ArtifactError::Storage);
    }
    let digest = format!("{:x}", hash.finalize());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o400))
            .map_err(|_| ArtifactError::Storage)?;
    }
    staged
        .as_file()
        .sync_all()
        .map_err(|_| ArtifactError::Storage)?;
    match staged.persist_noclobber(parent.join(format!("{digest}.bin"))) {
        Ok(_) => (),
        Err(e) if e.error.kind() == std::io::ErrorKind::AlreadyExists => {
            open(root, &digest, total)?;
        }
        Err(_) => return Err(ArtifactError::Storage),
    }
    fs::File::open(&parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| ArtifactError::Storage)?;
    Ok((digest, total))
}
pub(crate) fn open(root: &Path, digest: &str, size: u64) -> Result<fs::File> {
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        || size > MAX_BYTES
    {
        return Err(ArtifactError::Storage);
    }
    private(root)?;
    private(&root.join("objects"))?;
    let path: PathBuf = root.join("objects").join(format!("{digest}.bin"));
    let meta = fs::symlink_metadata(&path).map_err(|_| ArtifactError::Storage)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != size {
        return Err(ArtifactError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o277 != 0 {
            return Err(ArtifactError::Storage);
        }
    }
    fs::File::open(path).map_err(|_| ArtifactError::Storage)
}
