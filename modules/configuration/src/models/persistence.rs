// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Immutable revisions plus one atomic pointer; old Run secrets never mutate.
use super::{catalog, credentials};
use crate::{
    ConfigurationError, Result, files,
    validation::{fields, text},
};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub(super) struct Snapshot {
    pub catalog: Value,
    pub root: Option<PathBuf>,
    pub revision: u64,
}
pub(super) fn initialize(root: &Path) -> Result<()> {
    if !root.is_absolute() {
        return Err(ConfigurationError::PrivateFile);
    }
    if !root
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        directory(root.parent().ok_or(ConfigurationError::PrivateFile)?)?;
    }
    directory(root)?;
    let _lock = lock(root)?;
    if !root
        .join("models.json")
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        write(
            &root.join("models.json"),
            br#"{"providers":[],"models":[],"default_model":null,"default_models":{}}"#,
        )?;
        fs::File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|_| ConfigurationError::PrivateFile)?;
    }
    Ok(())
}
pub(super) fn load(base: &Path) -> Result<Snapshot> {
    files::private(base, true)?;
    let pointer = base.join("models-current.json");
    let (root, revision) = if pointer
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        let value = files::json(&pointer)?;
        fields(&value, &["generation", "revision"])?;
        let id = text(&value, "generation")?;
        let revision = value["revision"]
            .as_u64()
            .filter(|v| *v > 0)
            .ok_or(ConfigurationError::PrivateFile)?;
        if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(ConfigurationError::PrivateFile);
        }
        files::private(&base.join("model-versions"), true)?;
        (base.join("model-versions").join(id), revision)
    } else {
        (base.to_path_buf(), 0)
    };
    files::private(&root, true)?;
    let mut value = files::json(&root.join("models.json"))?;
    catalog::normalize(&mut value)?;
    catalog::validate(&value)?;
    credentials::count(Some(&root), &value)?;
    Ok(Snapshot {
        catalog: value,
        root: Some(root),
        revision,
    })
}
fn directory(path: &Path) -> Result<()> {
    if !path
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(path)
            .map_err(|_| ConfigurationError::PrivateFile)?;
    }
    files::private(path, true)
}
pub(super) fn lock(base: &Path) -> Result<fs::File> {
    files::private(base, true)?;
    let path = base.join("model-write.lock");
    if path
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        files::private(&path, false)?;
    }
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|_| ConfigurationError::PrivateFile)?;
    files::private(&path, false)?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|_| ConfigurationError::Conflict)?;
    Ok(file)
}
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| ConfigurationError::PrivateFile)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| ConfigurationError::PrivateFile)
}
/// The lock remains held through pointer fsync and in-memory publication.
pub(super) fn commit(
    base: &Path,
    previous: &Snapshot,
    value: Value,
    api_key: Option<(&str, &str)>,
) -> Result<Snapshot> {
    catalog::validate(&value)?;
    let bytes = serde_json::to_vec(&value).map_err(|_| ConfigurationError::Json)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(ConfigurationError::Invalid("model catalog exceeds 4 MiB"));
    }
    let revision = previous
        .revision
        .checked_add(1)
        .ok_or(ConfigurationError::Conflict)?;
    let parent = base.join("model-versions");
    directory(&parent)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(".pending-");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let staged = builder
        .tempdir_in(&parent)
        .map_err(|_| ConfigurationError::PrivateFile)?;
    write(&staged.path().join("models.json"), &bytes)?;
    directory(&staged.path().join("secrets"))?;
    directory(&staged.path().join("secrets/model-providers"))?;
    for provider in value["providers"]
        .as_array()
        .ok_or(ConfigurationError::Json)?
    {
        if provider["api_key_configured"].as_bool() != Some(true) {
            continue;
        }
        let name = text(provider, "name")?;
        let bytes = match api_key {
            Some((target, key)) if target == name => key.as_bytes().to_vec(),
            _ => credentials::read(
                previous
                    .root
                    .as_deref()
                    .ok_or(ConfigurationError::PrivateFile)?,
                name,
            )?,
        };
        write(
            &staged
                .path()
                .join("secrets/model-providers")
                .join(credentials::key_file(name)),
            &bytes,
        )?;
    }
    credentials::count(Some(staged.path()), &value)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for file in fs::read_dir(staged.path().join("secrets/model-providers"))
            .map_err(|_| ConfigurationError::PrivateFile)?
        {
            let path = file.map_err(|_| ConfigurationError::PrivateFile)?.path();
            fs::set_permissions(path, fs::Permissions::from_mode(0o400))
                .map_err(|_| ConfigurationError::PrivateFile)?;
        }
        fs::set_permissions(
            staged.path().join("models.json"),
            fs::Permissions::from_mode(0o400),
        )
        .map_err(|_| ConfigurationError::PrivateFile)?;
    }
    for dir in [
        staged.path().join("secrets/model-providers"),
        staged.path().join("secrets"),
        staged.path().into(),
    ] {
        fs::File::open(dir)
            .and_then(|f| f.sync_all())
            .map_err(|_| ConfigurationError::PrivateFile)?;
    }
    let mut nonce = [0u8; 32];
    getrandom::fill(&mut nonce).map_err(|_| ConfigurationError::PrivateFile)?;
    let generation = nonce.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let root = parent.join(&generation);
    fs::rename(staged.path(), &root).map_err(|_| ConfigurationError::PrivateFile)?;
    fs::File::open(&parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| ConfigurationError::PrivateFile)?;
    let mut pointer =
        tempfile::NamedTempFile::new_in(base).map_err(|_| ConfigurationError::PrivateFile)?;
    serde_json::to_writer(
        &mut pointer,
        &json!({"revision":revision,"generation":generation}),
    )
    .map_err(|_| ConfigurationError::Json)?;
    pointer
        .as_file()
        .sync_all()
        .map_err(|_| ConfigurationError::PrivateFile)?;
    pointer
        .persist(base.join("models-current.json"))
        .map_err(|_| ConfigurationError::PrivateFile)?;
    fs::File::open(base)
        .and_then(|f| f.sync_all())
        .map_err(|_| ConfigurationError::PrivateFile)?;
    Ok(Snapshot {
        catalog: value,
        root: Some(root),
        revision,
    })
}
