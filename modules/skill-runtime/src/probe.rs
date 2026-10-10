// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Trusted probe source. User package content is only argv JSON, never code.
use crate::{Result, RuntimeError, metadata::Requirements};
use exoharness_sandbox::{Limits, RunSpec, Runtime, Sandbox};
use serde::Deserialize;
use std::{collections::BTreeMap, time::Duration};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProbeResult {
    pub ready: bool,
    pub missing: Vec<String>,
    pub versions: BTreeMap<String, String>,
}
pub(crate) async fn check(sandbox: &Sandbox, metadata: &Requirements) -> Result<ProbeResult> {
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| RuntimeError::Unavailable)?;
    let id = format!("skill-environment-{:032x}", u128::from_le_bytes(random));
    let workspace = sandbox.create_workspace(
        &id,
        BTreeMap::from([
            ("check.py".into(), include_bytes!("probe.py").to_vec()),
            (
                "request.json".into(),
                serde_json::to_vec(metadata).map_err(|_| RuntimeError::Declaration)?,
            ),
        ]),
    )?;
    let result = sandbox
        .start(RunSpec {
            invocation_id: id,
            workspace,
            read_only_package: None,
            runtime: Runtime::Python,
            entrypoint: "check.py".into(),
            arguments: vec![],
            limits: Limits {
                timeout: Duration::from_secs(20),
                output_bytes: 64 * 1024,
                ..Limits::default()
            },
        })?
        .finish()
        .await?;
    if result.status != "completed" || result.output_truncated || !result.cleanup_confirmed {
        return Err(RuntimeError::Unavailable);
    }
    let probe: ProbeResult =
        serde_json::from_str(&result.stdout).map_err(|_| RuntimeError::Declaration)?;
    if probe.ready != probe.missing.is_empty()
        || probe.missing.len() > 256
        || probe.versions.len() > 256
        || probe.missing.iter().any(|v| v.len() > 512)
        || probe
            .versions
            .iter()
            .any(|(k, v)| k.len() > 256 || v.len() > 256)
    {
        return Err(RuntimeError::Unavailable);
    }
    Ok(probe)
}
