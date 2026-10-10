use crate::{McpError, Result};
use exoharness_contracts::capability::{EnvironmentBinding, StoredCapability};
use exoharness_secrets::SecretStore;
use http::{HeaderName, HeaderValue, header::AUTHORIZATION};
use std::collections::{BTreeMap, HashMap};
pub(crate) fn resolve(
    record: &StoredCapability,
    headers: &BTreeMap<String, EnvironmentBinding>,
    auth: Option<&str>,
    secrets: &SecretStore,
) -> Result<HashMap<HeaderName, HeaderValue>> {
    let mut values = HashMap::new();
    for (name, binding) in headers {
        let name = HeaderName::try_from(name.as_str()).map_err(|_| McpError::Secret)?;
        let mut value = match binding {
            EnvironmentBinding::Literal { value } => {
                HeaderValue::try_from(value.as_str()).map_err(|_| McpError::Secret)?
            }
            EnvironmentBinding::SecretRef { reference } => {
                HeaderValue::try_from(named(record, reference, secrets)?.expose())
                    .map_err(|_| McpError::Secret)?
            }
        };
        value.set_sensitive(true);
        values.insert(name, value);
    }
    if let Some(name) = auth {
        if values.contains_key(&AUTHORIZATION) {
            return Err(McpError::Secret);
        }
        let value = named(record, name, secrets)?;
        let full = if value.expose().split_once(' ').is_some() {
            value.expose().to_owned()
        } else {
            format!("Bearer {}", value.expose())
        };
        let mut value = HeaderValue::try_from(full).map_err(|_| McpError::Secret)?;
        value.set_sensitive(true);
        values.insert(AUTHORIZATION, value);
    }
    Ok(values)
}
fn named(
    record: &StoredCapability,
    name: &str,
    secrets: &SecretStore,
) -> Result<exoharness_secrets::SecretValue> {
    let reference = record
        .manifest
        .secret_refs
        .iter()
        .find(|r| r.name == name && r.provider == "local")
        .ok_or(McpError::Secret)?;
    secrets.get(&reference.key).map_err(|_| McpError::Secret)
}
