use crate::{Manager, Result, StoreError};
use r2d2::{Pool, PooledConnection};

/// Calls stay on blocking workers. Final synchronous driver cleanup also needs
/// a non-Tokio thread: postgres::Client::drop drives its private runtime.
pub(crate) struct OwnedPool(Option<Pool<Manager>>);
impl OwnedPool {
    pub fn new(pool: Pool<Manager>) -> Self {
        Self(Some(pool))
    }
    pub fn get(&self) -> Result<PooledConnection<Manager>> {
        self.0
            .as_ref()
            .expect("owned until drop")
            .get()
            .map_err(|_| StoreError::Connection)
    }
}
impl Drop for OwnedPool {
    fn drop(&mut self) {
        if let Some(pool) = self.0.take() {
            // Join rather than leaving connection teardown detached. This is
            // final owner teardown, not an HTTP request's query path.
            if std::thread::spawn(move || drop(pool)).join().is_err() {
                eprintln!("database pool cleanup failed");
            }
        }
    }
}
