CREATE TABLE IF NOT EXISTS execution_resource_reservations (
    invocation_id TEXT PRIMARY KEY REFERENCES execution_invocations(id) ON DELETE CASCADE,
    pool_id TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_execution_resource_pool ON execution_resource_reservations(pool_id);
