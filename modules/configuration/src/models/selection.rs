//! Catalog edits are pure; persistence and credentials are handled separately.
use crate::{ConfigurationError, Result, validation::text};
use serde_json::{Value, json};

pub(super) fn items<'a>(catalog: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    catalog[field].as_array().ok_or(ConfigurationError::Json)
}
pub(super) fn items_mut<'a>(catalog: &'a mut Value, field: &str) -> Result<&'a mut Vec<Value>> {
    catalog[field]
        .as_array_mut()
        .ok_or(ConfigurationError::Json)
}
pub(super) fn index(catalog: &Value, field: &str, name: &str) -> Result<usize> {
    items(catalog, field)?
        .iter()
        .position(|v| v["name"].as_str() == Some(name))
        .ok_or(ConfigurationError::NotFound)
}
pub(super) fn upsert(catalog: &mut Value, field: &str, mut value: Value) -> Result<Value> {
    match field {
        "models" => super::parameters::defaults(&mut value)?,
        "providers" => super::providers::defaults(&mut value)?,
        _ => return Err(ConfigurationError::Invalid("unknown catalog group")),
    }
    let name = text(&value, "name")?.to_owned();
    let items = items_mut(catalog, field)?;
    if let Some(existing) = items.iter_mut().find(|v| v["name"].as_str() == Some(&name)) {
        if super::kinds::model_type(existing)? != super::kinds::model_type(&value)? {
            return Err(ConfigurationError::Invalid(
                "cannot change configured model type",
            ));
        }
        *existing = value.clone();
    } else {
        if items.len() >= 1024 {
            return Err(ConfigurationError::Invalid("too many catalog entries"));
        }
        items.push(value.clone());
    }
    Ok(value)
}
pub(super) fn activate(catalog: &mut Value, name: &str) -> Result<()> {
    let index = index(catalog, "models", name)?;
    let kind = super::kinds::model_type(&catalog["models"][index])?.to_owned();
    let provider = text(&catalog["models"][index], "provider")?.to_owned();
    let provider_index = self::index(catalog, "providers", &provider)?;
    if catalog["providers"][provider_index]["enabled"].as_bool() == Some(false) {
        return Err(ConfigurationError::Invalid("model provider is disabled"));
    }
    for model in items_mut(catalog, "models")? {
        if super::kinds::model_type(model)? == kind {
            model["enabled"] = json!(model["name"].as_str() == Some(name));
        }
    }
    if catalog.get("default_models").is_none() {
        catalog["default_models"] = json!({});
    }
    let defaults = catalog["default_models"]
        .as_object_mut()
        .ok_or(ConfigurationError::Json)?;
    defaults.insert(kind.clone(), json!(name));
    if kind == "llm" {
        catalog["default_model"] = json!(name);
    }
    Ok(())
}
pub(super) fn remove_model(catalog: &mut Value, name: &str) -> Result<()> {
    let index = index(catalog, "models", name)?;
    items_mut(catalog, "models")?.remove(index);
    if catalog["default_model"].as_str() == Some(name) {
        catalog["default_model"] = Value::Null;
    }
    if let Some(defaults) = catalog
        .get_mut("default_models")
        .and_then(Value::as_object_mut)
    {
        defaults.retain(|_, v| v.as_str() != Some(name));
    }
    Ok(())
}
