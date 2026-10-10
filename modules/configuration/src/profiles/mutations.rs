use super::{Assistants, ProfileState, avatars, persistence};
use crate::{
    ConfigurationError, Result,
    validation::{fields, text},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn now() -> Result<u64> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| ConfigurationError::PrivateFile)?
            .as_millis(),
    )
    .map_err(|_| ConfigurationError::PrivateFile)
}
fn records(value: &mut Value) -> Result<&mut Vec<Value>> {
    value["profiles"]
        .as_array_mut()
        .ok_or(ConfigurationError::Json)
}
fn index(value: &Value, id: &str) -> Result<usize> {
    value["profiles"]
        .as_array()
        .ok_or(ConfigurationError::Json)?
        .iter()
        .position(|r| r["id"].as_str() == Some(id))
        .ok_or(ConfigurationError::NotFound)
}
fn active(record: &Value) -> Result<&Value> {
    record["revisions"]
        .as_array()
        .ok_or(ConfigurationError::Json)?
        .iter()
        .find(|r| r["revision"] == record["active_revision"])
        .ok_or(ConfigurationError::Json)
}
fn snapshot(
    id: &str,
    revision: u64,
    mut input: Value,
    previous: Option<&Value>,
    time: u64,
) -> Result<Value> {
    fields(
        &input,
        &[
            "display_name",
            "avatar_text",
            "avatar",
            "description",
            "system_prompt",
            "skill_ids",
            "access",
            "default_llm",
        ],
    )?;
    text(&json!({"id":id}), "id")?;
    if input.get("default_llm").is_none() {
        input["default_llm"] = previous
            .and_then(|p| p.get("default_llm"))
            .cloned()
            .unwrap_or(Value::Null);
    }
    if input.get("access").is_none_or(Value::is_null) {
        input["access"]=previous.and_then(|p|p.get("access")).filter(|v|!v.is_null()).cloned().unwrap_or_else(||json!({"skills":{"mode":"selected","ids":input.get("skill_ids").cloned().unwrap_or(json!([]))},"mcp":{"mode":"none"},"tools":{"mode":"none"},"models":{"mode":"none"},"published":false}));
    }
    super::access::validate(&input["access"])?;
    input["skill_ids"] =
        if input.pointer("/access/skills/mode").and_then(Value::as_str) == Some("selected") {
            input
                .pointer("/access/skills/ids")
                .cloned()
                .ok_or(ConfigurationError::Json)?
        } else {
            json!([])
        };
    input["id"] = json!(id);
    input["revision"] = json!(revision);
    input["created_at"] = json!(time);
    let digest = format!(
        "sha256:{:x}",
        Sha256::digest(serde_json::to_vec(&input).map_err(|_| ConfigurationError::Json)?)
    );
    input["digest"] = json!(digest);
    Ok(input)
}
impl Assistants {
    fn change<F, T>(&self, edit: F) -> Result<T>
    where
        F: FnOnce(&mut Value) -> Result<T>,
    {
        let root = self.root.as_deref().ok_or(ConfigurationError::Invalid(
            "assistant configuration has no persistent directory",
        ))?;
        let mut state = self
            .state
            .lock()
            .map_err(|_| ConfigurationError::PrivateFile)?;
        let _lease = persistence::lock(root)?;
        let mut value = persistence::load(root)?;
        let result = edit(&mut value)?;
        super::validation::validate(&value)?;
        let images = avatars::load(root, &value)?;
        persistence::commit(root, &value)?;
        *state = ProfileState {
            catalog: value,
            avatars: images,
        };
        Ok(result)
    }
    pub fn create(&self, id: &str, input: Value, enabled: bool, make_default: bool) -> Result<()> {
        self.change(|catalog| {
            if index(catalog,id).is_ok() {return Err(ConfigurationError::Conflict)}
            if make_default && !enabled {return Err(ConfigurationError::Invalid("default assistant must be enabled"))}
            let time=now()?;let value=snapshot(id,1,input,None,time)?;
            let profiles=records(catalog)?;
            if profiles.len()>=256 {return Err(ConfigurationError::Invalid("too many assistants"))}
            profiles.push(json!({"id":id,"enabled":enabled,"active_revision":1,"created_at":time,"updated_at":time,"revisions":[value]}));
            if make_default {catalog["default_profile"]=json!(id);}
            Ok(())
        })
    }
    pub fn update(&self, id: &str, expected: u64, input: Value) -> Result<()> {
        self.change(|catalog| {
            let index = index(catalog, id)?;
            let record = &catalog["profiles"][index];
            let revision = record["active_revision"]
                .as_u64()
                .ok_or(ConfigurationError::Json)?;
            if revision != expected {
                return Err(ConfigurationError::Conflict);
            }
            let next = revision
                .checked_add(1)
                .ok_or(ConfigurationError::Conflict)?;
            let time = now()?;
            let value = snapshot(id, next, input, Some(active(record)?), time)?;
            let record = &mut catalog["profiles"][index];
            record["revisions"]
                .as_array_mut()
                .ok_or(ConfigurationError::Json)?
                .push(value);
            record["active_revision"] = json!(next);
            record["updated_at"] = json!(time);
            Ok(())
        })
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<()> {
        self.change(|catalog| {
            let index = index(catalog, id)?;
            if !enabled && catalog["default_profile"].as_str() == Some(id) {
                return Err(ConfigurationError::Conflict);
            }
            catalog["profiles"][index]["enabled"] = json!(enabled);
            catalog["profiles"][index]["updated_at"] = json!(now()?);
            Ok(())
        })
    }
    pub fn set_default(&self, id: &str) -> Result<()> {
        self.change(|catalog| {
            let index = index(catalog, id)?;
            if catalog["profiles"][index]["enabled"] != true {
                return Err(ConfigurationError::Invalid(
                    "default assistant must be enabled",
                ));
            }
            catalog["default_profile"] = json!(id);
            Ok(())
        })
    }
    /// The reference check runs under the same lease as user assignment.
    pub fn remove<F>(&self, id: &str, referenced: F) -> Result<()>
    where
        F: FnOnce() -> Result<bool>,
    {
        self.change(|catalog| {
            let index = index(catalog, id)?;
            if catalog["default_profile"].as_str() == Some(id) || referenced()? {
                return Err(ConfigurationError::Conflict);
            }
            records(catalog)?.remove(index);
            Ok(())
        })
    }
}
