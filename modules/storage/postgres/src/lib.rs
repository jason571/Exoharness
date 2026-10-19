// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

//! Transactional persistence boundary. Synchronous driver calls must run on a
//! dedicated blocking worker, never directly inside an async HTTP/model loop.
mod artifacts;
mod chat;
mod codec;
mod model_observation;
mod task_context;
pub use artifacts::StoredArtifact;
mod control;
mod conversations;
mod external_sessions;
pub use external_sessions::ExternalSessionRequest;
mod external_conversations;
pub use external_conversations::ExternalConversationQuery;
mod invocations;
mod mcp_admission;
pub use mcp_admission::InvocationAdmission;
mod mcp_reconciliation;
pub use mcp_reconciliation::PendingMcpTask;
mod journal;
mod knowledge;
mod overview;
mod pool;
mod registry;
mod runs;
mod scheduler;
mod sessions;
mod trace;
mod user_activity;
pub use registry::RegistryAccess;

pub use exoharness_contracts::execution::*;
use postgres::{Config, NoTls};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
pub use sessions::CreateTurn;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("database connection/configuration failed")]
    Connection,
    #[error("database operation failed")]
    Database,
    #[error("persistent state could not be decoded")]
    Decode,
    #[error("record not found in the authorized scope")]
    NotFound,
    #[error("execution state or immutable identity conflicts")]
    Conflict,
    #[error("invalid execution input: {0}")]
    Invalid(&'static str),
    #[error("snapshot does not authorize this capability/tool")]
    NotAuthorized,
    #[error("external outcome requires reconciliation before continuing")]
    OutcomeUnknown,
    #[error("consecutive identical call limit reached")]
    RepeatedCall,
    #[error("execution was requested but no executable capability succeeded")]
    MissingExecution,
    #[error("execution capacity is occupied; wait without resubmitting")]
    CapacityWaiting,
    #[error("legacy Run requires explicit snapshot migration; it will not be executed implicitly")]
    MigrationRequired,
}

impl From<postgres::Error> for StoreError {
    fn from(_: postgres::Error) -> Self {
        Self::Database
    }
}
impl From<serde_json::Error> for StoreError {
    fn from(_: serde_json::Error) -> Self {
        Self::Decode
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;
type Manager = PostgresConnectionManager<NoTls>;

#[derive(Clone)]
pub struct PostgresStore {
    pool: Arc<pool::OwnedPool>,
}

impl PostgresStore {
    /// Loopback/database-private-network adapter. TLS-aware remote database
    /// transport is a separate composition decision; this is not a public URL.
    pub fn connect(url: &str, namespace: &str) -> Result<Self> {
        Self::connect_with_password(url, namespace, None)
    }

    fn connect_with_password(url: &str, namespace: &str, password: Option<&str>) -> Result<Self> {
        if namespace.is_empty()
            || namespace.len() > 63
            || !namespace
                .bytes()
                .enumerate()
                .all(|(i, c)| c == b'_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit()))
        {
            return Err(StoreError::Invalid(
                "database namespace must be a SQL identifier",
            ));
        }
        let mut config = Config::from_str(url).map_err(|_| StoreError::Connection)?;
        if let Some(password) = password {
            config.password(password);
        }
        let mut bootstrap = config.connect(NoTls).map_err(|_| StoreError::Connection)?;
        bootstrap.batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {namespace}"))?;
        config.options(&format!("-c search_path={namespace},public -c statement_timeout=30000 -c lock_timeout=5000 -c idle_in_transaction_session_timeout=60000"));
        let manager = PostgresConnectionManager::new(config, NoTls);
        let pool = Pool::builder()
            .max_size(8)
            // Do not reserve eight idle connections for every module/test
            // namespace. Open on demand; the bootstrap already checked access.
            .min_idle(Some(0))
            .build(manager)
            .map_err(|_| StoreError::Connection)?;
        Ok(Self {
            pool: Arc::new(pool::OwnedPool::new(pool)),
        })
    }

    pub fn migrate(&self) -> Result<()> {
        let mut connection = self.connection()?;
        let mut tx = connection.transaction()?;
        // Migrations in one namespace serialize; independent installations
        // must not contend on a database-wide lock and exhaust lock_timeout.
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended(current_schema(),4271072001))",
            &[],
        )?;
        tx.batch_execute(include_str!("../migrations/0001_legacy_execution.sql"))?;
        tx.batch_execute(include_str!("../migrations/0002_knowledge.sql"))?;
        tx.batch_execute(include_str!("../migrations/0003_document_import.sql"))?;
        tx.batch_execute(include_str!("../migrations/0004_conversation_titles.sql"))?;
        tx.batch_execute(include_str!("../migrations/0005_scheduler.sql"))?;
        tx.batch_execute(include_str!("../migrations/0006_external_sessions.sql"))?;
        tx.batch_execute(include_str!(
            "../migrations/0007_external_session_devices.sql"
        ))?;
        tx.commit()?;
        Ok(())
    }

    pub fn health(&self) -> Result<()> {
        self.connection()?.query_one("SELECT 1", &[])?;
        Ok(())
    }

    /// Same connection variable and optional password-file convention as the
    /// existing ExoHarness. Never print either the URL or password contents.
    pub fn from_env(namespace: &str) -> Result<Self> {
        let url = std::env::var("EXOHARNESS_DATABASE_URL").map_err(|_| StoreError::Connection)?;
        if let Some(path) = std::env::var_os("EXOHARNESS_DATABASE_PASSWORD_FILE") {
            let password = std::fs::read_to_string(path).map_err(|_| StoreError::Connection)?;
            let password = password.trim_end_matches(['\r', '\n']);
            if password.is_empty() {
                return Err(StoreError::Connection);
            }
            Self::connect_with_password(&url, namespace, Some(password))
        } else {
            Self::connect(&url, namespace)
        }
    }

    fn connection(&self) -> Result<r2d2::PooledConnection<Manager>> {
        self.pool.get()
    }
}
