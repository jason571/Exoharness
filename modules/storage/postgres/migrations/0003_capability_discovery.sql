-- Original capability_health lacks revision and cannot prove live schemas.
-- Keep the original table; attach immutable schema evidence to its version.
CREATE TABLE IF NOT EXISTS capability_discovery (
    capability_id TEXT NOT NULL,
    revision BIGINT NOT NULL,
    schema_digest TEXT NOT NULL,
    tools_json TEXT NOT NULL,
    server_info_json TEXT NOT NULL,
    observed_at BIGINT NOT NULL,
    PRIMARY KEY(capability_id, revision),
    FOREIGN KEY(capability_id,revision) REFERENCES capability_versions(capability_id,revision) ON DELETE CASCADE
);
