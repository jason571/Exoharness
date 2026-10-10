// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{
    Result, SecretError, SecretValue,
    filesystem::{directory, key_valid, private_file},
};
use exoharness_core::services::Service;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};
pub struct SecretStore {
    root: PathBuf,
    changes: Mutex<()>,
}
impl Service for SecretStore {
    const KEY: &'static str = "secrets";
}
impl SecretStore {
    pub fn open(root: PathBuf) -> Result<Self> {
        // Only one known private parent may be created. Do not recursively
        // traverse arbitrary user paths or relax permissions on existing data.
        if !root.try_exists()? {
            directory(root.parent().ok_or(SecretError::Invalid)?)?;
        }
        directory(&root)?;
        Ok(Self {
            root,
            changes: Mutex::new(()),
        })
    }
    pub fn put(&self, value: &str) -> Result<String> {
        valid(value)?;
        let _guard = self.changes.lock().map_err(|_| SecretError::Storage)?;
        directory(&self.root)?;
        let mut random = [0u8; 32];
        getrandom::fill(&mut random).map_err(|_| SecretError::Storage)?;
        let key = random
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(self.path(&key)?)?;
        file.write_all(value.as_bytes())?;
        file.sync_all()?;
        sync(&self.root)?;
        Ok(key)
    }
    pub fn get(&self, key: &str) -> Result<SecretValue> {
        let _guard = self.changes.lock().map_err(|_| SecretError::Storage)?;
        directory(&self.root)?;
        let path = self.path(key)?;
        private_file(&path)?;
        let mut value = String::new();
        fs::File::open(path)?
            .take(8193)
            .read_to_string(&mut value)?;
        valid(&value)?;
        Ok(SecretValue(value))
    }
    pub fn configured(&self, key: &str) -> bool {
        self.get(key).is_ok()
    }
    pub fn rotate(&self, key: &str, value: &str) -> Result<()> {
        valid(value)?;
        let _guard = self.changes.lock().map_err(|_| SecretError::Storage)?;
        directory(&self.root)?;
        let path = self.path(key)?;
        private_file(&path)?;
        let mut builder = tempfile::Builder::new();
        builder.prefix(".rotation-");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(fs::Permissions::from_mode(0o600));
        }
        let mut temp = builder.tempfile_in(&self.root)?;
        temp.write_all(value.as_bytes())?;
        temp.as_file().sync_all()?;
        temp.persist(path).map_err(|_| SecretError::Storage)?;
        sync(&self.root)
    }
    pub fn delete(&self, key: &str) -> Result<()> {
        let _guard = self.changes.lock().map_err(|_| SecretError::Storage)?;
        directory(&self.root)?;
        let path = self.path(key)?;
        match private_file(&path) {
            Ok(()) => {}
            Err(SecretError::Missing) => return Ok(()),
            Err(error) => return Err(error),
        }
        fs::remove_file(path)?;
        sync(&self.root)
    }
    fn path(&self, key: &str) -> Result<PathBuf> {
        if !key_valid(key) {
            return Err(SecretError::Invalid);
        }
        Ok(self.root.join(format!("{key}.secret")))
    }
}
fn valid(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 8192 || value.chars().any(char::is_control) {
        return Err(SecretError::Invalid);
    }
    Ok(())
}
fn sync(path: &Path) -> Result<()> {
    fs::File::open(path)?.sync_all()?;
    Ok(())
}
