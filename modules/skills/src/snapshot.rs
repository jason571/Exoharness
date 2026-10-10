// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Verified server-side package bytes; never a public filesystem handle.
use exoharness_contracts::capability::StoredCapability;
use std::collections::BTreeMap;

pub struct PackageSnapshot {
    pub capability: StoredCapability,
    pub files: BTreeMap<String, Vec<u8>>,
}
