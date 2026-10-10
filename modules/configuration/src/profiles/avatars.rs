// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Private, content-addressed avatars; no base64 is stored in configuration.
use super::{Assistants, persistence};
use crate::{ConfigurationError, Result, files, validation::asset};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, io::Write, path::Path};

type AvatarMap = BTreeMap<String, (String, Vec<u8>)>;
const LIMIT: usize = 2 * 1024 * 1024;
fn mime(bytes: &[u8], kind: &str) -> bool {
    match kind {
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/webp" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP",
        _ => false,
    }
}
fn read(root: &Path, id: &str, kind: &str) -> Result<Vec<u8>> {
    asset(&json!({"asset_id":id,"content_type":kind}))?;
    files::private(&root.join("agent-assets"), true)?;
    let bytes = files::read(&root.join("agent-assets").join(id), LIMIT as u64)?;
    if format!("{:x}", Sha256::digest(&bytes)) != id.split('.').next().unwrap_or("")
        || !mime(&bytes, kind)
    {
        return Err(ConfigurationError::Invalid(
            "avatar digest or MIME mismatch",
        ));
    }
    Ok(bytes)
}
pub(super) fn load(root: &Path, catalog: &Value) -> Result<AvatarMap> {
    let mut avatars = BTreeMap::new();
    let mut total = 0usize;
    for record in catalog["profiles"]
        .as_array()
        .ok_or(ConfigurationError::Invalid("profiles must be an array"))?
    {
        for revision in record["revisions"]
            .as_array()
            .ok_or(ConfigurationError::Invalid("revisions must be an array"))?
        {
            if let Some(value) = revision.get("avatar").filter(|value| !value.is_null()) {
                let (id, kind) = asset(value)?;
                if avatars.contains_key(id) {
                    continue;
                }
                let bytes = read(root, id, kind)?;
                total = total
                    .checked_add(bytes.len())
                    .ok_or(ConfigurationError::PrivateFile)?;
                if total > 64 * 1024 * 1024 {
                    return Err(ConfigurationError::Invalid("avatar cache exceeds 64 MiB"));
                }
                avatars.insert(id.into(), (kind.into(), bytes));
            }
        }
    }
    Ok(avatars)
}
impl Assistants {
    pub fn upload_avatar(&self, kind: &str, bytes: &[u8]) -> Result<Value> {
        if bytes.is_empty() || bytes.len() > LIMIT || !mime(bytes, kind) {
            return Err(ConfigurationError::Invalid(
                "avatar must be a PNG, JPEG or WebP image up to 2 MiB",
            ));
        }
        let extension = match kind {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            _ => return Err(ConfigurationError::Invalid("invalid avatar MIME")),
        };
        let id = format!("{:x}.{extension}", Sha256::digest(bytes));
        let root = self
            .root
            .as_deref()
            .ok_or(ConfigurationError::PrivateFile)?;
        let _state = self
            .state
            .lock()
            .map_err(|_| ConfigurationError::PrivateFile)?;
        let _lease = persistence::lock(root)?;
        let directory = root.join("agent-assets");
        persistence::directory(&directory)?;
        let path = directory.join(&id);
        if path
            .try_exists()
            .map_err(|_| ConfigurationError::PrivateFile)?
        {
            read(root, &id, kind)?;
        } else {
            let mut temp = tempfile::NamedTempFile::new_in(&directory)
                .map_err(|_| ConfigurationError::PrivateFile)?;
            temp.write_all(bytes)
                .and_then(|_| {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        temp.as_file()
                            .set_permissions(fs::Permissions::from_mode(0o400))?;
                    }
                    Ok(())
                })
                .and_then(|_| temp.as_file().sync_all())
                .map_err(|_| ConfigurationError::PrivateFile)?;
            temp.persist_noclobber(path)
                .map_err(|_| ConfigurationError::PrivateFile)?;
            fs::File::open(&directory)
                .and_then(|f| f.sync_all())
                .map_err(|_| ConfigurationError::PrivateFile)?;
        }
        Ok(json!({"asset_id":id,"content_type":kind}))
    }
    pub fn avatar(&self, id: &str) -> Option<(String, Vec<u8>)> {
        if let Some(value) = self.state.lock().ok()?.avatars.get(id).cloned() {
            return Some(value);
        }
        let kind = match id.rsplit_once('.')?.1 {
            "png" => "image/png",
            "jpg" => "image/jpeg",
            "webp" => "image/webp",
            _ => return None,
        };
        read(self.root.as_deref()?, id, kind)
            .ok()
            .map(|bytes| (kind.into(), bytes))
    }
    pub fn avatar_count(&self) -> usize {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .avatars
            .len()
    }
}
