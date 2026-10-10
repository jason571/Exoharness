// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Recoverable filesystem half of Skill deletion; IDs, never supplied paths.
use super::{Packages, directory, real_directory, temporary};
use crate::{Result, errors::validation};
use exoharness_registry::Registry;
use exoharness_storage_postgres::{RegistryAccess, StoreError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    id: String,
}
pub(crate) struct Removal {
    live: PathBuf,
    trash: PathBuf,
    rollback: bool,
}
impl Removal {
    pub(crate) fn commit(mut self) -> Result<()> {
        self.rollback = false;
        fs::remove_dir_all(&self.trash)?;
        fs::File::open(self.trash.parent().expect("parent"))?.sync_all()?;
        Ok(())
    }
}
impl Drop for Removal {
    fn drop(&mut self) {
        if !self.rollback {
            return;
        }
        let saved = self.trash.join("packages");
        if saved.exists() && fs::rename(&saved, &self.live).is_err() {
            return;
        }
        let _ = fs::remove_dir_all(&self.trash);
    }
}
impl Packages {
    pub(crate) fn prepare_removal(&self, id: &str) -> Result<Removal> {
        let removing = self.root.join("removing");
        directory(&removing)?;
        // Hidden staging has no live package and can be discarded after a crash.
        // Only a fully synced intent is published into the recovery namespace.
        let staging = temporary(&removing, ".intent-")?;
        let trash = removing.join(format!(
            "remove-{}",
            staging
                .path()
                .file_name()
                .expect("staging name")
                .to_string_lossy()
        ));
        let live = self
            .root
            .join("packages")
            .join(format!("{:x}", Sha256::digest(id.as_bytes())));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(staging.path().join("intent.json"))?;
        let data = serde_json::to_vec(&Intent { id: id.into() })
            .map_err(|_| validation("删除记录无效"))?;
        file.write_all(&data)?;
        file.sync_all()?;
        fs::File::open(staging.path())?.sync_all()?;
        fs::rename(staging.path(), &trash)?;
        fs::File::open(&removing)?.sync_all()?;
        let removal = Removal {
            live,
            trash,
            rollback: true,
        };
        if removal.live.try_exists()? {
            real_directory(&removal.live)?;
            fs::rename(&removal.live, removal.trash.join("packages"))?;
            fs::File::open(removal.live.parent().expect("parent"))?.sync_all()?;
            fs::File::open(&removal.trash)?.sync_all()?;
        }
        Ok(removal)
    }
    pub(crate) fn recover_removals(&self, registry: &Registry) -> Result<()> {
        let root = self.root.join("removing");
        real_directory(&root)?;
        for item in fs::read_dir(&root)? {
            let trash = item?.path();
            real_directory(&trash)?;
            if trash
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".intent-"))
            {
                // A staging directory cannot contain a moved live package.
                if trash.join("packages").try_exists()? {
                    return Err(validation("Skill 删除准备日志需要核对"));
                }
                fs::remove_dir_all(&trash)?;
                continue;
            }
            let mut text = String::new();
            fs::File::open(trash.join("intent.json"))?
                .take(4097)
                .read_to_string(&mut text)?;
            if text.len() > 4096 {
                return Err(validation("Skill 删除日志过大"));
            }
            let value = exoharness_contracts::strict_json::parse(&text)
                .map_err(|_| validation("Skill 删除日志损坏"))?;
            let intent: Intent =
                serde_json::from_value(value).map_err(|_| validation("Skill 删除日志无效"))?;
            if intent.id.is_empty() || intent.id.len() > 512 {
                return Err(validation("Skill 删除身份无效"));
            }
            let live = self
                .root
                .join("packages")
                .join(format!("{:x}", Sha256::digest(intent.id.as_bytes())));
            match registry
                .0
                .capability(&RegistryAccess::Administrator, &intent.id)
            {
                Ok(_) => {
                    let saved = trash.join("packages");
                    if saved.try_exists()? {
                        real_directory(&saved)?;
                        if live.try_exists()? {
                            return Err(validation("Skill 删除恢复需要核对，未覆盖现有包"));
                        }
                        fs::rename(saved, &live)?;
                        fs::File::open(live.parent().expect("parent"))?.sync_all()?;
                    }
                }
                Err(StoreError::NotFound) => {}
                Err(error) => return Err(error.into()),
            }
            fs::remove_dir_all(trash)?;
            fs::File::open(&root)?.sync_all()?;
        }
        Ok(())
    }
}
