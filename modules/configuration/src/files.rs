use crate::{ConfigurationError, Result};
use exoharness_contracts::strict_json;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(crate) fn private(path: &Path, directory: bool) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| ConfigurationError::PrivateFile)?;
    if metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        return Err(ConfigurationError::PrivateFile);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(ConfigurationError::PrivateFile);
        }
    }
    Ok(())
}

pub(crate) fn read(path: &Path, limit: u64) -> Result<Vec<u8>> {
    private(path, false)?;
    if std::fs::metadata(path)
        .map_err(|_| ConfigurationError::PrivateFile)?
        .len()
        > limit
    {
        return Err(ConfigurationError::PrivateFile);
    }
    let bytes = std::fs::read(path).map_err(|_| ConfigurationError::PrivateFile)?;
    if bytes.len() as u64 > limit {
        return Err(ConfigurationError::PrivateFile);
    }
    Ok(bytes)
}

pub(crate) fn json(path: &Path) -> Result<Value> {
    let bytes = read(path, 4 * 1024 * 1024)?;
    strict_json::parse(std::str::from_utf8(&bytes).map_err(|_| ConfigurationError::Json)?)
        .map_err(|_| ConfigurationError::Json)
}

/// One pointer selects an immutable configuration generation. Never source
/// server.env; do not inherit production database/binds/origins implicitly.
pub fn selected_generation(base: &Path) -> Result<Option<PathBuf>> {
    if !base.exists() {
        return Ok(None);
    }
    private(base, true)?;
    let pointer = base.join("current.json");
    if !pointer.exists() {
        return Ok(None);
    }
    let value = json(&pointer)?;
    let name = value
        .get("generation")
        .and_then(Value::as_str)
        .filter(|name| {
            !name.is_empty()
                && name.len() <= 128
                && name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
        .ok_or(ConfigurationError::Invalid("invalid generation identifier"))?;
    let root = base.join(name);
    private(&root, true)?;
    Ok(Some(root))
}
