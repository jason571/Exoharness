//! One registry projection; the transport adapter may not fabricate records
//! from assistant selections or claim execution readiness from registration.
use exoharness_contracts::capability::RegisteredCapabilityView;
use exoharness_core::services::Service;
use exoharness_storage_postgres::{PostgresStore, RegistryAccess};

pub struct Registry(pub PostgresStore);
impl Service for Registry {
    const KEY: &'static str = "capability-registry";
}
impl Registry {
    pub fn views(
        &self,
        access: &RegistryAccess,
        limit: usize,
    ) -> exoharness_storage_postgres::Result<Vec<RegisteredCapabilityView>> {
        self.0.capability_views(access, limit)
    }
}
