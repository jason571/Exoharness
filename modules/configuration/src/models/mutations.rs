use super::{Models, persistence, selection};
use crate::{ConfigurationError, Result, validation::text};
use serde_json::{Value, json};

impl Models {
    fn change<F>(
        &self,
        expected: Option<u64>,
        api_key: Option<(&str, &str)>,
        edit: F,
    ) -> Result<Value>
    where
        F: FnOnce(&mut Value) -> Result<Value>,
    {
        if let Some((_, key)) = api_key
            && (key.is_empty() || key.len() > 65536 || key.contains(['\0', '\r', '\n']))
        {
            return Err(ConfigurationError::Invalid("invalid API key"));
        }
        let base = self.root.as_deref().ok_or(ConfigurationError::Invalid(
            "model configuration has no persistent directory",
        ))?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ConfigurationError::PrivateFile)?;
        let _lock = persistence::lock(base)?;
        let current = persistence::load(base)?;
        if expected.is_some_and(|v| v != current.revision) {
            return Err(ConfigurationError::Conflict);
        }
        let mut value = current.catalog.clone();
        let response = edit(&mut value)?;
        super::catalog::validate(&value)?;
        if api_key.is_none() && value == current.catalog {
            *state = current;
            return Ok(response);
        }
        *state = persistence::commit(base, &current, value, api_key)?;
        Ok(response)
    }
    pub fn save_configuration(
        &self,
        mut provider: Value,
        model: Value,
        api_key: Option<&str>,
        expected: Option<u64>,
    ) -> Result<Value> {
        let name = text(&provider, "name")?.to_owned();
        if text(&model, "provider")? != name {
            return Err(ConfigurationError::Invalid(
                "configuration provider mismatch",
            ));
        }
        let key = api_key.filter(|key| !key.is_empty());
        let secret_name = name.clone();
        self.change(expected,key.map(|key|(secret_name.as_str(),key)),move|catalog| {
            provider["api_key_configured"]=json!(key.is_some() || selection::items(catalog,"providers")?.iter().any(|v|v["name"].as_str()==Some(&name) && v["api_key_configured"].as_bool()==Some(true)));
            if provider["api_key_configured"]!=true {return Err(ConfigurationError::Invalid("API key is required"))}
            let provider=selection::upsert(catalog,"providers",provider)?;
            let model=selection::upsert(catalog,"models",model)?;
            let model_name=text(&model,"name")?;
            if model["enabled"].as_bool().unwrap_or(true) {selection::activate(catalog,model_name)?;}
            Ok(json!({"provider":provider,"model":catalog["models"][selection::index(catalog,"models",model_name)?],"default_model":catalog["default_model"],"default_models":catalog["default_models"]}))
        })
    }
    pub fn save_provider(&self, mut provider: Value, expected: Option<u64>) -> Result<Value> {
        let name = text(&provider, "name")?.to_owned();
        self.change(expected, None, move |catalog| {
            provider["api_key_configured"] = json!(
                selection::items(catalog, "providers")?
                    .iter()
                    .any(|v| v["name"].as_str() == Some(&name)
                        && v["api_key_configured"].as_bool() == Some(true))
            );
            selection::upsert(catalog, "providers", provider)
        })
    }
    pub fn save_model(&self, model: Value, expected: Option<u64>) -> Result<Value> {
        self.change(expected, None, move |catalog| {
            let model = selection::upsert(catalog, "models", model)?;
            let name = text(&model, "name")?;
            if model["enabled"].as_bool().unwrap_or(true) {
                selection::activate(catalog, name)?;
            }
            Ok(catalog["models"][selection::index(catalog, "models", name)?].clone())
        })
    }
    pub fn set_model_enabled(
        &self,
        name: &str,
        enabled: bool,
        expected: Option<u64>,
    ) -> Result<Value> {
        self.change(expected, None, |catalog| {
            let index = selection::index(catalog, "models", name)?;
            if enabled {
                selection::activate(catalog, name)?;
            } else {
                catalog["models"][index]["enabled"] = json!(false);
            }
            Ok(catalog["models"][index].clone())
        })
    }
    pub fn set_default(&self, name: &str, expected: Option<u64>) -> Result<Value> {
        self.set_model_enabled(name, true, expected)
    }
    pub fn set_provider_enabled(
        &self,
        name: &str,
        enabled: bool,
        expected: Option<u64>,
    ) -> Result<Value> {
        self.change(expected, None, |catalog| {
            let index = selection::index(catalog, "providers", name)?;
            catalog["providers"][index]["enabled"] = json!(enabled);
            Ok(catalog["providers"][index].clone())
        })
    }
    pub fn remove_model(&self, name: &str, expected: Option<u64>) -> Result<()> {
        self.change(expected, None, |catalog| {
            selection::remove_model(catalog, name)?;
            Ok(Value::Null)
        })
        .map(|_| ())
    }
    pub fn remove_provider(&self, name: &str, expected: Option<u64>) -> Result<()> {
        self.change(expected, None, |catalog| {
            if selection::items(catalog, "models")?
                .iter()
                .any(|v| v["provider"].as_str() == Some(name))
            {
                return Err(ConfigurationError::Conflict);
            }
            let index = selection::index(catalog, "providers", name)?;
            selection::items_mut(catalog, "providers")?.remove(index);
            Ok(Value::Null)
        })
        .map(|_| ())
    }
}
