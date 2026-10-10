// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use crate::params::translate_placeholders;
use crate::{Connection, Error, Params, Result, Transaction};
use postgres::{Client, types::FromSqlOwned};
pub(crate) enum StatementExecutor<'a> {
    Connection(&'a Connection),
    Transaction(&'a Transaction),
}

pub struct Statement<'a> {
    pub(crate) executor: StatementExecutor<'a>,
    pub(crate) sql: String,
}

impl Statement<'_> {
    pub fn query_map<P, T, F>(&mut self, params: P, mut mapper: F) -> Result<MappedRows<T>>
    where
        P: Params + Send,
        T: Send,
        F: FnMut(&Row) -> Result<T> + Send,
    {
        let sql = translate_placeholders(&self.sql);
        let params = params.as_params();
        let map_rows = |client: &mut Client| -> Result<MappedRows<T>> {
            let rows = client.query(&sql, &params)?;
            let mapped = rows
                .into_iter()
                .map(|row| mapper(&Row(row)))
                .collect::<Vec<_>>();
            Ok(MappedRows(mapped.into_iter()))
        };
        match self.executor {
            StatementExecutor::Connection(connection) => connection.with_client(map_rows),
            StatementExecutor::Transaction(transaction) => transaction.with_client(map_rows),
        }
    }

    pub fn query_row<P, T, F>(&mut self, params: P, mapper: F) -> Result<T>
    where
        P: Params + Send,
        T: Send,
        F: FnOnce(&Row) -> Result<T> + Send,
    {
        match self.executor {
            StatementExecutor::Connection(connection) => {
                connection.query_row(&self.sql, params, mapper)
            }
            StatementExecutor::Transaction(transaction) => {
                transaction.query_row(&self.sql, params, mapper)
            }
        }
    }
}

pub struct MappedRows<T>(std::vec::IntoIter<Result<T>>);

impl<T> Iterator for MappedRows<T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }
}

pub struct Row(pub(crate) postgres::Row);

impl Row {
    pub fn get<I, T>(&self, index: I) -> Result<T>
    where
        I: DatabaseRowIndex,
        T: FromSqlOwned,
    {
        index.try_get(&self.0).map_err(Error::from)
    }
}

pub trait DatabaseRowIndex {
    fn try_get<T: FromSqlOwned>(
        self,
        row: &postgres::Row,
    ) -> std::result::Result<T, postgres::Error>;
}

impl DatabaseRowIndex for usize {
    fn try_get<T: FromSqlOwned>(
        self,
        row: &postgres::Row,
    ) -> std::result::Result<T, postgres::Error> {
        row.try_get(self)
    }
}

impl DatabaseRowIndex for &str {
    fn try_get<T: FromSqlOwned>(
        self,
        row: &postgres::Row,
    ) -> std::result::Result<T, postgres::Error> {
        row.try_get(self)
    }
}

pub trait OptionalExtension<T> {
    fn optional(self) -> Result<Option<T>>;
}

impl<T> OptionalExtension<T> for Result<T> {
    fn optional(self) -> Result<Option<T>> {
        match self {
            Ok(value) => Ok(Some(value)),
            Err(Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error),
        }
    }
}
