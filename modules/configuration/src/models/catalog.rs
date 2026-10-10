use crate::{ConfigurationError, Result, validation::fields};
use serde_json::Value;

/// Preserve the typed old API contract when a valid client omits defaulted
/// fields. This never writes the immutable imported source document.
pub(super) fn normalize(catalog: &mut Value) -> Result<()> {
    for provider in catalog["providers"]
        .as_array_mut()
        .ok_or(ConfigurationError::Json)?
    {
        super::providers::defaults(provider)?;
    }
    for model in catalog["models"]
        .as_array_mut()
        .ok_or(ConfigurationError::Json)?
    {
        super::parameters::defaults(model)?;
    }
    Ok(())
}

pub(super) fn validate(catalog: &Value) -> Result<()> {
    fields(
        catalog,
        &["providers", "models", "default_model", "default_models"],
    )?;
    let providers = catalog
        .get("providers")
        .and_then(Value::as_array)
        .ok_or(ConfigurationError::Invalid("providers must be an array"))?;
    let models = catalog
        .get("models")
        .and_then(Value::as_array)
        .ok_or(ConfigurationError::Invalid("models must be an array"))?;
    super::providers::validate(providers)?;
    super::parameters::validate(models, providers)?;
    super::defaults::validate(catalog, models)
}
