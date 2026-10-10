//! Mature PostgreSQL SQL parameter adapter, reused without the legacy API.
mod connection;
mod params;
mod pool;
mod rows;
mod transaction;
pub use connection::{Connection, Executor};
pub use params::Params;
pub use pool::{
    DATABASE_PASSWORD_FILE_ENV, DATABASE_URL_ENV, Database, TEST_DATABASE_PASSWORD_FILE_ENV,
    TEST_DATABASE_URL_ENV,
};
pub use postgres::types;
pub use rows::{DatabaseRowIndex, MappedRows, OptionalExtension, Row, Statement};
use thiserror::Error;
pub use transaction::Transaction;
#[derive(Debug, Error)]
pub enum Error {
    #[error("PostgreSQL configuration is invalid: {0}")]
    Configuration(String),
    #[error("PostgreSQL connection pool failed: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("PostgreSQL operation failed: {}", postgres_error_message(.0))]
    Postgres(#[from] postgres::Error),
    #[error("database query returned no rows")]
    QueryReturnedNoRows,
    #[error("database value could not be decoded: {0}")]
    Decode(String),
}

pub type Result<T> = std::result::Result<T, Error>;

fn postgres_error_message(error: &postgres::Error) -> String {
    error.as_db_error().map_or_else(
        || error.to_string(),
        |details| format!("{} ({})", details.message(), details.code().code()),
    )
}

#[cfg(test)]
#[path = "../tests/unit/parameters.rs"]
mod tests;
