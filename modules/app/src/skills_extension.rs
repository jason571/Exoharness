// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use async_trait::async_trait;
use exoharness_core::{
    graph::ExtensionSpec,
    lifecycle::Extension,
    services::{ExtensionContext, Service},
};
use exoharness_registry::Registry;
use exoharness_skills::Skills;
use std::{path::PathBuf, sync::Arc};

pub struct SkillsExtension {
    pub root: PathBuf,
}
#[async_trait]
impl Extension for SkillsExtension {
    fn spec(&self) -> ExtensionSpec {
        ExtensionSpec {
            id: "skills".into(),
            provides: vec![Skills::KEY.into()],
            requires: vec![Registry::KEY.into()],
        }
    }
    async fn start(&mut self, context: &mut ExtensionContext<'_>) -> Result<(), String> {
        let registry = context.require::<Registry>().map_err(|e| e.to_string())?;
        let root = self.root.clone();
        let skills = tokio::task::spawn_blocking(move || Skills::open(root, registry))
            .await
            .map_err(|_| "Skill storage worker failed")?
            .map_err(|e| e.to_string())?;
        context.provide(Arc::new(skills)).map_err(|e| e.to_string())
    }
    async fn stop(&mut self) -> Result<(), String> {
        Ok(())
    }
}
