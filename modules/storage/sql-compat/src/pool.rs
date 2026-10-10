// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::{Connection, Error, Result};
use postgres::{Config, NoTls};
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use std::{
    env, fs,
    path::Path,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
pub(crate) type Manager = PostgresConnectionManager<NoTls>;
pub const DATABASE_URL_ENV: &str = "EXOHARNESS_DATABASE_URL";
pub const DATABASE_PASSWORD_FILE_ENV: &str = "EXOHARNESS_DATABASE_PASSWORD_FILE";
pub const TEST_DATABASE_URL_ENV: &str = "EXOHARNESS_PRO_TEST_DATABASE_URL";
pub const TEST_DATABASE_PASSWORD_FILE_ENV: &str = "EXOHARNESS_PRO_TEST_DATABASE_PASSWORD_FILE";
#[derive(Clone, Debug)]
pub struct Database {
    pub(crate) inner: Arc<DatabaseInner>,
}

#[derive(Debug)]
pub(crate) struct DatabaseInner {
    pool: Option<Pool<Manager>>,
    namespace: Arc<str>,
}

impl DatabaseInner {
    pub(crate) fn pool(&self) -> &Pool<Manager> {
        self.pool.as_ref().expect("database pool is alive")
    }
}

impl Drop for DatabaseInner {
    fn drop(&mut self) {
        if let Some(pool) = self.pool.take() {
            outside_async_runtime(|| drop(pool));
        }
    }
}

impl Database {
    pub fn from_env(namespace: &str) -> Result<Self> {
        let url = env::var(DATABASE_URL_ENV).map_err(|_| {
            Error::Configuration(format!(
                "{DATABASE_URL_ENV} is required; ExoHarness no longer starts with an implicit SQLite database"
            ))
        })?;
        let password_file = env::var_os(DATABASE_PASSWORD_FILE_ENV).map(std::path::PathBuf::from);
        Self::connect(&url, password_file.as_deref(), namespace)
    }

    pub fn connect(url: &str, password_file: Option<&Path>, namespace: &str) -> Result<Self> {
        validate_namespace(namespace)?;
        let mut config =
            Config::from_str(url).map_err(|error| Error::Configuration(error.to_string()))?;
        if let Some(path) = password_file {
            let password = fs::read_to_string(path).map_err(|error| {
                Error::Configuration(format!(
                    "could not read {} from {}: {error}",
                    DATABASE_PASSWORD_FILE_ENV,
                    path.display()
                ))
            })?;
            let password = password.trim_end_matches(['\r', '\n']);
            if password.is_empty() {
                return Err(Error::Configuration(format!(
                    "{} must not be empty",
                    path.display()
                )));
            }
            config.password(password);
        }
        outside_async_runtime(|| {
            let mut bootstrap = config.connect(NoTls)?;
            bootstrap.batch_execute(&format!("CREATE SCHEMA IF NOT EXISTS {namespace}"))?;
            drop(bootstrap);
            Result::<_>::Ok(())
        })?;
        config.options(&format!(
            "-c search_path={namespace},public -c statement_timeout=30000 -c lock_timeout=5000 -c idle_in_transaction_session_timeout=60000"
        ));
        let manager = PostgresConnectionManager::new(config, NoTls);
        let pool =
            outside_async_runtime(|| Pool::builder().max_size(8).min_idle(Some(0)).build(manager))?;
        let database = Self {
            inner: Arc::new(DatabaseInner {
                pool: Some(pool),
                namespace: Arc::from(namespace),
            }),
        };
        database.connection()?;
        Ok(database)
    }

    pub fn test(namespace: &str) -> Result<Self> {
        static NEXT_SCHEMA: AtomicU64 = AtomicU64::new(1);
        let suffix = NEXT_SCHEMA.fetch_add(1, Ordering::Relaxed);
        let namespace = format!("test_{namespace}_{}_{}", std::process::id(), suffix);
        let url = env::var(TEST_DATABASE_URL_ENV)
            .map_err(|_| Error::Configuration("isolated test database URL is required".into()))?;
        let password_file = env::var_os(TEST_DATABASE_PASSWORD_FILE_ENV)
            .or_else(|| env::var_os(DATABASE_PASSWORD_FILE_ENV))
            .map(std::path::PathBuf::from);
        Self::connect(&url, password_file.as_deref(), &namespace)
    }

    pub fn connection(&self) -> Result<Connection> {
        // Check out once here so startup fails early if PostgreSQL is not
        // reachable; later operations obtain their own pooled connection.
        outside_async_runtime(|| self.inner.pool().get().map(drop))?;
        Ok(Connection {
            database: self.clone(),
        })
    }

    pub fn namespace(&self) -> &str {
        &self.inner.namespace
    }
}

pub(crate) fn outside_async_runtime<T, F>(work: F) -> T
where
    T: Send,
    F: FnOnce() -> T + Send,
{
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return work();
    };
    if handle.runtime_flavor() == tokio::runtime::RuntimeFlavor::MultiThread {
        return tokio::task::block_in_place(work);
    }
    std::thread::scope(|scope| match scope.spawn(work).join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    })
}

fn validate_namespace(namespace: &str) -> Result<()> {
    if namespace.is_empty()
        || namespace.len() > 63
        || !namespace.bytes().enumerate().all(|(index, byte)| {
            byte == b'_' || byte.is_ascii_lowercase() || (index > 0 && byte.is_ascii_digit())
        })
    {
        return Err(Error::Configuration(format!(
            "invalid PostgreSQL namespace {namespace:?}"
        )));
    }
    Ok(())
}
