use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSpec {
    pub id: String,
    /// Interfaces with a version in their name, for example `journal/v1`.
    pub provides: Vec<String>,
    pub requires: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GraphError {
    #[error("duplicate extension: {0}")]
    DuplicateExtension(String),
    #[error("multiple providers for service: {0}")]
    DuplicateProvider(String),
    #[error("extension {extension} requires unavailable service {service}")]
    MissingService { extension: String, service: String },
    #[error("dependency cycle among extensions: {0:?}")]
    Cycle(Vec<String>),
    #[error("extension and service identifiers must not be blank")]
    EmptyIdentifier,
}

/// Validate the entire dependency graph BEFORE creating any resources.
/// Stable ordering makes startup, shutdown and replay tests deterministic.
pub fn activation_order(specs: &[ExtensionSpec]) -> Result<Vec<String>, GraphError> {
    let mut by_id = BTreeMap::new();
    let mut providers = BTreeMap::new();
    for spec in specs {
        if spec.id.trim().is_empty()
            || spec
                .provides
                .iter()
                .chain(&spec.requires)
                .any(|id| id.trim().is_empty())
        {
            return Err(GraphError::EmptyIdentifier);
        }
        if by_id.insert(spec.id.clone(), spec).is_some() {
            return Err(GraphError::DuplicateExtension(spec.id.clone()));
        }
        for service in &spec.provides {
            if providers.insert(service.clone(), spec.id.clone()).is_some() {
                return Err(GraphError::DuplicateProvider(service.clone()));
            }
        }
    }
    let mut pending = BTreeMap::new();
    for spec in specs {
        let mut dependencies = BTreeSet::new();
        for service in &spec.requires {
            let provider = providers
                .get(service)
                .ok_or_else(|| GraphError::MissingService {
                    extension: spec.id.clone(),
                    service: service.clone(),
                })?;
            dependencies.insert(provider.clone());
        }
        pending.insert(spec.id.clone(), dependencies);
    }
    let mut ready = BTreeSet::new();
    let mut order = Vec::new();
    while !pending.is_empty() {
        let next = pending
            .iter()
            .find(|(_, dependencies)| dependencies.is_subset(&ready))
            .map(|(id, _)| id.clone());
        let Some(next) = next else {
            return Err(GraphError::Cycle(pending.keys().cloned().collect()));
        };
        pending.remove(&next);
        ready.insert(next.clone());
        order.push(next);
    }
    Ok(order)
}
