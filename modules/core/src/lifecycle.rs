// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::graph::{ExtensionSpec, GraphError, activation_order};
use crate::resources::Resources;
use crate::services::{ExtensionContext, Registry, Service, ServiceError};
use async_trait::async_trait;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Extensions own all their allocations, including partial startup resources.
/// stop() must drain async resources and must be safe after a failed start.
/// The host does not pretend Drop can await asynchronous cleanup.
#[async_trait]
pub trait Extension: Send {
    fn spec(&self) -> ExtensionSpec;
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String>;
    async fn stop(&mut self) -> Result<(), String>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostState {
    Idle,
    Starting,
    Ready,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleEvent {
    Starting(String),
    Ready(String),
    StartFailed { extension: String, error: String },
    Stopping(String),
    Stopped(String),
    StopFailed { extension: String, error: String },
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Graph(#[from] GraphError),
    #[error("host is not idle: {0:?}")]
    NotIdle(HostState),
    #[error("host is not ready: {0:?}")]
    NotReady(HostState),
    #[error(transparent)]
    Service(#[from] ServiceError),
    #[error("extension {extension} failed startup: {error}; rollback errors: {rollback:?}")]
    Start {
        extension: String,
        error: String,
        rollback: Vec<(String, String)>,
    },
}

/// Single lifecycle owner; mutable access serializes registration/start/stop.
/// No replacement or registration is permitted during teardown. Service calls
/// and live reconfiguration will be added only after lease semantics are tested.
pub struct Host {
    state: HostState,
    extensions: BTreeMap<String, Registered>,
    services: Registry,
    resources: Resources,
    owned: Vec<String>,
    events: Vec<LifecycleEvent>,
}

struct Registered {
    spec: ExtensionSpec,
    extension: Box<dyn Extension>,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            state: HostState::Idle,
            extensions: BTreeMap::new(),
            services: Registry::default(),
            resources: Resources::default(),
            owned: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl Host {
    pub fn state(&self) -> HostState {
        self.state
    }
    pub fn events(&self) -> &[LifecycleEvent] {
        &self.events
    }
    pub fn take_events(&mut self) -> Vec<LifecycleEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn service<T: Service>(&self) -> Result<Arc<T>, HostError> {
        if self.state != HostState::Ready {
            return Err(HostError::NotReady(self.state));
        }
        Ok(self.services.get::<T>()?)
    }

    pub fn register(&mut self, extension: Box<dyn Extension>) -> Result<(), HostError> {
        if self.state != HostState::Idle {
            return Err(HostError::NotIdle(self.state));
        }
        let spec = extension.spec();
        if spec.id.trim().is_empty() {
            return Err(GraphError::EmptyIdentifier.into());
        }
        if self.extensions.contains_key(&spec.id) {
            return Err(GraphError::DuplicateExtension(spec.id).into());
        }
        self.extensions
            .insert(spec.id.clone(), Registered { spec, extension });
        Ok(())
    }

    pub async fn start(&mut self) -> Result<(), HostError> {
        if self.state != HostState::Idle {
            return Err(HostError::NotIdle(self.state));
        }
        let specs = self
            .extensions
            .values()
            .map(|extension| extension.spec.clone())
            .collect::<Vec<_>>();
        let order = activation_order(&specs)?;
        self.state = HostState::Starting;
        for id in order {
            // Record ownership before awaiting setup. If this future is dropped,
            // shutdown can still clean up the partial extension and dependencies.
            self.owned.push(id.clone());
            self.events.push(LifecycleEvent::Starting(id.clone()));
            let registered = self.extensions.get_mut(&id).expect("validated extension");
            let mut context = ExtensionContext {
                spec: &registered.spec,
                registry: &mut self.services,
                resources: &mut self.resources,
            };
            let result = registered.extension.start(&mut context).await;
            let result = result.and_then(|()| {
                self.services
                    .validate_publications(&registered.spec)
                    .map_err(|error| error.to_string())
            });
            if let Err(error) = result {
                self.events.push(LifecycleEvent::StartFailed {
                    extension: id.clone(),
                    error: error.clone(),
                });
                let rollback = self.shutdown().await;
                self.state = HostState::Failed;
                return Err(HostError::Start {
                    extension: id,
                    error,
                    rollback,
                });
            }
            self.events.push(LifecycleEvent::Ready(id));
        }
        self.state = HostState::Ready;
        Ok(())
    }

    /// Reverse dependency teardown. Independent siblings still clean up after
    /// failure, but prerequisites of a failed cleanup must remain owned/live.
    /// Ownership remains recorded while awaiting stop, so cancelling shutdown
    /// cannot silently forget the extension still being cleaned up.
    pub async fn shutdown(&mut self) -> Vec<(String, String)> {
        self.state = HostState::Stopping;
        let mut errors = Vec::new();
        let mut blocked = BTreeSet::new();
        for id in self.owned.clone().into_iter().rev() {
            if blocked.contains(&id) {
                self.block_prerequisites(&id, &mut blocked);
                continue;
            }
            self.events.push(LifecycleEvent::Stopping(id.clone()));
            let stopped = self
                .extensions
                .get_mut(&id)
                .expect("owned extension")
                .extension
                .stop()
                .await;
            let stopped = match stopped {
                Ok(()) => self.resources.release(&id).await,
                Err(error) => Err(error),
            };
            match stopped {
                Ok(()) => {
                    self.events.push(LifecycleEvent::Stopped(id.clone()));
                    self.services.revoke(&id);
                    self.owned.retain(|owner| owner != &id);
                }
                Err(error) => {
                    self.events.push(LifecycleEvent::StopFailed {
                        extension: id.clone(),
                        error: error.clone(),
                    });
                    self.block_prerequisites(&id, &mut blocked);
                    errors.push((id, error));
                }
            }
        }
        self.state = if errors.is_empty() {
            HostState::Idle
        } else {
            HostState::Failed
        };
        errors
    }

    fn block_prerequisites(&self, id: &str, blocked: &mut BTreeSet<String>) {
        for key in &self.extensions[id].spec.requires {
            if let Some((provider, _)) = self
                .extensions
                .iter()
                .find(|(_, registered)| registered.spec.provides.contains(key))
            {
                blocked.insert(provider.clone());
            }
        }
    }
}
