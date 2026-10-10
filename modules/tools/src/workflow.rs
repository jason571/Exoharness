//! Resolve declared durable steps against the authorized frozen catalog.
use crate::{Binding, Catalog};
use serde_json::Value;
use std::collections::BTreeSet;

impl Catalog {
    pub fn provider_tools(&self, source: &Binding) -> Vec<String> {
        self.bindings
            .values()
            .filter(|b| b.capability == source.capability && b.revision == source.revision)
            .map(|b| b.tool.name.clone())
            .collect()
    }

    /// Metadata selects schemas, never grants access or runs a tool. A missing
    /// or ambiguous dependency keeps the normal catalog/context unchanged.
    pub fn resolve_workflow(&self, source: &Binding, directive: &Value) -> Option<Vec<String>> {
        if directive["version"] != 1 || directive["context_scope"] != "checkpoint" {
            return None;
        }
        if directive["status"] == "complete" {
            return Some(vec![]);
        }
        if directive["status"] != "required" || !directive["checkpoint"].is_object() {
            return None;
        }
        let required = directive["required_tool"].as_str()?;
        let main = self.resolve_step(required, Some(&source.capability))?;
        if main.capability != source.capability || main.revision != source.revision {
            return None;
        }
        if let Some(arguments) = directive.get("required_arguments") {
            if !arguments.is_object() {
                return None;
            }
            let mut schema = main.tool.input_schema.clone();
            schema.as_object_mut()?.remove("required");
            crate::validate(&schema, arguments).ok()?;
        }
        let mut names = BTreeSet::from([main.tool.name.clone()]);
        if let Some(dependencies) = directive.get("prerequisite_tools") {
            let dependencies = dependencies.as_array()?;
            if dependencies.len() > 32 {
                return None;
            }
            for dependency in dependencies {
                let (name, scope) = if let Some(name) = dependency.as_str() {
                    (name, None)
                } else {
                    (
                        dependency["tool"].as_str()?,
                        dependency["capability_id"].as_str(),
                    )
                };
                names.insert(self.resolve_step(name, scope)?.tool.name.clone());
            }
        }
        Some(names.into_iter().collect())
    }

    fn resolve_step(&self, name: &str, scope: Option<&str>) -> Option<&Binding> {
        // Historical capability IDs are hints only when that provider is absent.
        // Resolve a unique authorized raw name, not a newly manufactured alias.
        let known_scope =
            scope.filter(|scope| self.bindings.values().any(|b| b.capability == *scope));
        let mut candidates = self.bindings.values().filter(|b| {
            (b.raw_name == name || b.tool.name == name)
                && known_scope.is_none_or(|scope| b.capability == scope)
        });
        let binding = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        Some(binding)
    }

    /// This reversible view retains the discovery/reload entry points. Loaded
    /// schemas remain available to explicit subsequent activation.
    pub fn workflow_schemas(
        &mut self,
        names: &[String],
    ) -> Option<Vec<exoharness_contracts::model::Tool>> {
        if names.iter().any(|name| !self.bindings.contains_key(name)) {
            return None;
        }
        self.active.extend(names.iter().cloned());
        let mut schemas = crate::native::base();
        schemas.extend(names.iter().map(|name| self.bindings[name].tool.clone()));
        Some(schemas)
    }
}
