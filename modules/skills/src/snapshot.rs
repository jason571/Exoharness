//! Verified server-side package bytes; never a public filesystem handle.
use exoharness_contracts::capability::StoredCapability;
use std::collections::BTreeMap;

pub struct PackageSnapshot {
    pub capability: StoredCapability,
    pub files: BTreeMap<String, Vec<u8>>,
}
