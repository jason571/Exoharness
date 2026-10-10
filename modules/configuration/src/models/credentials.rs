//! Private credential checks, never a public configuration projection.
use crate::{ConfigurationError, Result, files, validation::text};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;

pub(super) fn count(root: Option<&Path>, catalog: &Value) -> Result<usize> {
    let Some(root) = root else { return Ok(0) };
    let mut count = 0;
    for provider in catalog["providers"]
        .as_array()
        .ok_or(ConfigurationError::Invalid("providers must be an array"))?
    {
        if provider.get("api_key_configured").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let directory = root.join("secrets/model-providers");
        files::private(&root.join("secrets"), true)?;
        files::private(&directory, true)?;
        let key = key_file(text(provider, "name")?);
        let data = files::read(&directory.join(key), 65536)?;
        let secret = std::str::from_utf8(&data).map_err(|_| ConfigurationError::PrivateFile)?;
        if secret.trim().is_empty() || secret.contains(['\0', '\r', '\n']) {
            return Err(ConfigurationError::PrivateFile);
        }
        count += 1;
    }
    Ok(count)
}
pub(super) fn key_file(name: &str) -> String {
    format!(
        "model-provider-{:x}.secret",
        Sha256::digest(name.as_bytes())
    )
}
pub(super) fn read(root: &Path, name: &str) -> Result<Vec<u8>> {
    files::private(&root.join("secrets"), true)?;
    files::private(&root.join("secrets/model-providers"), true)?;
    files::read(
        &root.join("secrets/model-providers").join(key_file(name)),
        65536,
    )
}
