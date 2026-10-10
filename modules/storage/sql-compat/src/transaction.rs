use crate::{Database, Error, Executor, Params, Result, Row, Statement};
use crate::{
    params::{execute, query_row},
    pool::{Manager, outside_async_runtime},
    rows::StatementExecutor,
};
use postgres::Client;
use r2d2::PooledConnection;
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
pub struct Transaction {
    client: Mutex<Option<PooledConnection<Manager>>>,
    finished: AtomicBool,
}

impl Transaction {
    pub(crate) fn begin(database: Database) -> Result<Self> {
        let client = outside_async_runtime(|| {
            let mut client = database.inner.pool().get()?;
            client.batch_execute("BEGIN")?;
            Result::<_>::Ok(client)
        })?;
        Ok(Self {
            client: Mutex::new(Some(client)),
            finished: AtomicBool::new(false),
        })
    }

    pub(crate) fn with_client<T, F>(&self, callback: F) -> Result<T>
    where
        T: Send,
        F: FnOnce(&mut Client) -> Result<T> + Send,
    {
        outside_async_runtime(|| {
            let mut slot = self
                .client
                .lock()
                .map_err(|_| Error::Configuration("database transaction is poisoned".into()))?;
            let client = slot.as_mut().ok_or_else(|| {
                Error::Configuration("database transaction is already complete".into())
            })?;
            callback(client)
        })
    }

    pub fn execute<P: Params + Send>(&self, sql: &str, params: P) -> Result<usize> {
        self.with_client(|client| execute(client, sql, params))
    }

    pub fn execute_batch(&self, sql: &str) -> Result<()> {
        self.with_client(|client| {
            client.batch_execute(sql)?;
            Ok(())
        })
    }

    pub fn query_row<P, T, F>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params + Send,
        T: Send,
        F: FnOnce(&Row) -> Result<T> + Send,
    {
        self.with_client(|client| query_row(client, sql, params, mapper))
    }

    pub fn prepare<'a>(&'a self, sql: &str) -> Result<Statement<'a>> {
        Ok(Statement {
            executor: StatementExecutor::Transaction(self),
            sql: sql.to_owned(),
        })
    }

    pub fn commit(self) -> Result<()> {
        self.with_client(|client| {
            client.batch_execute("COMMIT")?;
            Ok(())
        })?;
        self.finished.store(true, Ordering::Release);
        outside_async_runtime(|| {
            let client = self
                .client
                .lock()
                .map_err(|_| Error::Configuration("database transaction is poisoned".into()))?
                .take();
            drop(client);
            Result::<_>::Ok(())
        })?;
        Ok(())
    }
}

impl Executor for Transaction {
    fn execute<P: Params + Send>(&self, sql: &str, params: P) -> Result<usize> {
        Transaction::execute(self, sql, params)
    }

    fn query_row<P, T, F>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params + Send,
        T: Send,
        F: FnOnce(&Row) -> Result<T> + Send,
    {
        Transaction::query_row(self, sql, params, mapper)
    }

    fn prepare<'a>(&'a self, sql: &str) -> Result<Statement<'a>> {
        Transaction::prepare(self, sql)
    }
}

impl Drop for Transaction {
    fn drop(&mut self) {
        if !self.finished.load(Ordering::Acquire) {
            if let Ok(slot) = self.client.get_mut()
                && let Some(client) = slot.as_mut()
            {
                let _ = outside_async_runtime(|| client.batch_execute("ROLLBACK"));
            }
            if let Ok(slot) = self.client.get_mut() {
                let client = slot.take();
                outside_async_runtime(|| drop(client));
            }
        }
    }
}
