//! One atomic current document retains immutable per-assistant snapshots.
use crate::{ConfigurationError, Result, files};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

pub(super) fn directory(path: &Path) -> Result<()> {
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
    let _lease = lock(root)?;
    if !root
        .join("agent-profiles.json")
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        atomic(
            root,
            "agent-profiles.json",
            &json!({"profiles":[],"default_profile":null}),
        )?;
    }
    directory(&root.join("agent-assets"))
}
pub(super) fn lock(root: &Path) -> Result<fs::File> {
    files::private(root, true)?;
    let path = root.join("assistant-write.lock");
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
pub(super) fn load(root: &Path) -> Result<Value> {
    files::private(root, true)?;
    let current = root.join("agent-profiles-current.json");
    let path = if current
        .try_exists()
        .map_err(|_| ConfigurationError::PrivateFile)?
    {
        current
    } else {
        root.join("agent-profiles.json")
    };
    let value = files::json(&path)?;
    super::validation::validate(&value)?;
    Ok(value)
}
fn atomic(root: &Path, name: &str, value: &Value) -> Result<()> {
    let data = serde_json::to_vec(value).map_err(|_| ConfigurationError::Json)?;
    if data.len() > 4 * 1024 * 1024 {
        return Err(ConfigurationError::Invalid(
            "assistant catalog exceeds 4 MiB",
        ));
    }
    let mut staged =
        tempfile::NamedTempFile::new_in(root).map_err(|_| ConfigurationError::PrivateFile)?;
    staged
        .write_all(&data)
        .and_then(|_| staged.as_file().sync_all())
        .map_err(|_| ConfigurationError::PrivateFile)?;
    staged
        .persist(root.join(name))
        .map_err(|_| ConfigurationError::PrivateFile)?;
    fs::File::open(root)
        .and_then(|f| f.sync_all())
        .map_err(|_| ConfigurationError::PrivateFile)
}
pub(super) fn commit(root: &Path, value: &Value) -> Result<()> {
    super::validation::validate(value)?;
    atomic(root, "agent-profiles-current.json", value)
}
