use crate::{Database, Params, Result, Row, Statement, Transaction};
use crate::{
    params::{execute, query_row},
    pool::outside_async_runtime,
    rows::StatementExecutor,
};
use postgres::Client;
#[derive(Clone)]
pub struct Connection {
    pub(crate) database: Database,
}

impl Connection {
    pub(crate) fn with_client<T, F>(&self, callback: F) -> Result<T>
    where
        T: Send,
        F: FnOnce(&mut Client) -> Result<T> + Send,
    {
        outside_async_runtime(|| {
            let mut client = self.database.inner.pool().get()?;
            callback(&mut client)
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
            executor: StatementExecutor::Connection(self),
            sql: sql.to_owned(),
        })
    }

    pub fn transaction(&self) -> Result<Transaction> {
        Transaction::begin(self.database.clone())
    }

    pub fn unchecked_transaction(&self) -> Result<Transaction> {
        self.transaction()
    }
}

pub trait Executor {
    fn execute<P: Params + Send>(&self, sql: &str, params: P) -> Result<usize>;
    fn query_row<P, T, F>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params + Send,
        T: Send,
        F: FnOnce(&Row) -> Result<T> + Send;
    fn prepare<'a>(&'a self, sql: &str) -> Result<Statement<'a>>;
}

impl Executor for Connection {
    fn execute<P: Params + Send>(&self, sql: &str, params: P) -> Result<usize> {
        Connection::execute(self, sql, params)
    }

    fn query_row<P, T, F>(&self, sql: &str, params: P, mapper: F) -> Result<T>
    where
        P: Params + Send,
        T: Send,
        F: FnOnce(&Row) -> Result<T> + Send,
    {
        Connection::query_row(self, sql, params, mapper)
    }

    fn prepare<'a>(&'a self, sql: &str) -> Result<Statement<'a>> {
        Connection::prepare(self, sql)
    }
}
