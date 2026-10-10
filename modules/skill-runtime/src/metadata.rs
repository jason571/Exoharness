// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Result, RuntimeError};
use exoharness_skills::PackageSnapshot;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Serialize)]
pub(crate) struct Requirements {
    pub runtimes: BTreeSet<&'static str>,
    pub requirements: Option<String>,
    pub pyproject: Option<String>,
    pub package: Option<String>,
}
pub(crate) fn extract(package: &PackageSnapshot) -> Result<Requirements> {
    let mut runtimes = BTreeSet::new();
    for path in package.files.keys() {
        if let Some(kind) = match std::path::Path::new(path)
            .extension()
            .and_then(|e| e.to_str())
        {
            Some("py") => Some("python"),
            Some("sh") => Some("shell"),
            Some("js" | "mjs" | "cjs") => Some("javascript"),
            Some("ts" | "mts" | "cts") => Some("typescript"),
            _ => None,
        } {
            runtimes.insert(kind);
        }
    }
    let text = |name: &str| -> Result<Option<String>> {
        package
            .files
            .get(name)
            .map(|bytes| {
                if bytes.len() > 64 * 1024 {
                    return Err(RuntimeError::Declaration);
                }
                std::str::from_utf8(bytes)
                    .map(str::to_owned)
                    .map_err(|_| RuntimeError::Declaration)
            })
            .transpose()
    };
    Ok(Requirements {
        runtimes,
        requirements: text("requirements.txt")?,
        pyproject: text("pyproject.toml")?,
        package: text("package.json")?,
    })
}
