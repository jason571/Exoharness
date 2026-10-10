// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Private immutable-image/revision proof cache; never an execution grant.
use crate::{EnvironmentEvidence, Result, RuntimeError};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub(crate) struct Cache {
    root: PathBuf,
}
impl Cache {
    pub fn open(root: PathBuf) -> Result<Self> {
        if !root.is_absolute() {
            return Err(RuntimeError::Storage);
        }
        if !root.try_exists().map_err(|_| RuntimeError::Storage)? {
            verify(root.parent().ok_or(RuntimeError::Storage)?)?;
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root).map_err(|_| RuntimeError::Storage)?;
        }
        verify(&root)?;
        Ok(Self { root })
    }
    fn path(&self, id: &str, revision: i64, digest: &str, image: &str) -> PathBuf {
        // Length-delimited JSON avoids collisions between opaque input fields.
        let key = serde_json::to_vec(&(id, revision, digest, image)).expect("string tuple");
        self.root.join(format!("{:x}.json", Sha256::digest(key)))
    }
    pub fn read(
        &self,
        id: &str,
        revision: i64,
        digest: &str,
        image: &str,
    ) -> Result<Option<EnvironmentEvidence>> {
        verify(&self.root)?;
        let path = self.path(id, revision, digest, image);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(value) => value,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(RuntimeError::Storage),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 64 * 1024 {
            return Err(RuntimeError::Storage);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(RuntimeError::Storage);
            }
        }
        let mut bytes = Vec::new();
        fs::File::open(&path)
            .map_err(|_| RuntimeError::Storage)?
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| RuntimeError::Storage)?;
        let value: EnvironmentEvidence =
            serde_json::from_slice(&bytes).map_err(|_| RuntimeError::Storage)?;
        if value.capability_id != id
            || value.revision != revision
            || value.source_digest != digest
            || value.image_id != image
            || value.ready != value.missing.is_empty()
        {
            return Err(RuntimeError::Storage);
        }
        Ok(Some(value))
    }
    pub fn write(&self, value: &EnvironmentEvidence) -> Result<()> {
        verify(&self.root)?;
        let bytes = serde_json::to_vec(value).map_err(|_| RuntimeError::Storage)?;
        if bytes.len() > 64 * 1024 {
            return Err(RuntimeError::Storage);
        }
        let mut temp =
            tempfile::NamedTempFile::new_in(&self.root).map_err(|_| RuntimeError::Storage)?;
        temp.write_all(&bytes)
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|_| RuntimeError::Storage)?;
        temp.persist(self.path(
            &value.capability_id,
            value.revision,
            &value.source_digest,
            &value.image_id,
        ))
        .map_err(|_| RuntimeError::Storage)?;
        fs::File::open(&self.root)
            .and_then(|f| f.sync_all())
            .map_err(|_| RuntimeError::Storage)
    }
}
fn verify(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| RuntimeError::Storage)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || fs::canonicalize(path).map_err(|_| RuntimeError::Storage)? != path
    {
        return Err(RuntimeError::Storage);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(RuntimeError::Storage);
        }
    }
    Ok(())
}
