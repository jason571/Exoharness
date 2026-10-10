// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::graph::ExtensionSpec;
use crate::resources::{OwnedResource, Resources};
use std::{any::Any, collections::BTreeMap, sync::Arc};

pub trait Service: Any + Send + Sync {
    const KEY: &'static str;
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ServiceError {
    #[error("service {0} was not declared by this extension")]
    Undeclared(String),
    #[error("service {0} is unavailable")]
    Unavailable(String),
    #[error("service {0} already has a provider")]
    Duplicate(String),
    #[error("service {0} has an incompatible Rust type")]
    TypeMismatch(String),
}

struct Slot {
    owner: String,
    value: Arc<dyn Any + Send + Sync>,
}

#[derive(Default)]
pub(crate) struct Registry(BTreeMap<String, Slot>);

impl Registry {
    pub(crate) fn get<T: Service>(&self) -> Result<Arc<T>, ServiceError> {
        self.0
            .get(T::KEY)
            .ok_or_else(|| ServiceError::Unavailable(T::KEY.into()))?
            .value
            .clone()
            .downcast::<T>()
            .map_err(|_| ServiceError::TypeMismatch(T::KEY.into()))
    }

    pub(crate) fn revoke(&mut self, owner: &str) {
        self.0.retain(|_, slot| slot.owner != owner);
    }

    pub(crate) fn validate_publications(&self, spec: &ExtensionSpec) -> Result<(), ServiceError> {
        for key in &spec.provides {
            if !self.0.get(key).is_some_and(|slot| slot.owner == spec.id) {
                return Err(ServiceError::Unavailable(key.clone()));
            }
        }
        Ok(())
    }
}

/// A restricted startup context, not a bag of application globals. Providers
/// can publish only declared interfaces and resolve only declared dependencies.
pub struct ExtensionContext<'a> {
    pub(crate) spec: &'a ExtensionSpec,
    pub(crate) registry: &'a mut Registry,
    pub(crate) resources: &'a mut Resources,
}

impl ExtensionContext<'_> {
    pub fn own(&mut self, name: &str, resource: Box<dyn OwnedResource>) -> Result<(), String> {
        self.resources.own(&self.spec.id, name, resource)
    }

    pub fn require<T: Service>(&self) -> Result<Arc<T>, ServiceError> {
        if !self.spec.requires.iter().any(|key| key == T::KEY) {
            return Err(ServiceError::Undeclared(T::KEY.into()));
        }
        self.registry.get::<T>()
    }

    pub fn provide<T: Service>(&mut self, value: Arc<T>) -> Result<(), ServiceError> {
        if !self.spec.provides.iter().any(|key| key == T::KEY) {
            return Err(ServiceError::Undeclared(T::KEY.into()));
        }
        if self.registry.0.contains_key(T::KEY) {
            return Err(ServiceError::Duplicate(T::KEY.into()));
        }
        self.registry.0.insert(
            T::KEY.into(),
            Slot {
                owner: self.spec.id.clone(),
                value,
            },
        );
        Ok(())
    }
}
