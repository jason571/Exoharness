CREATE TABLE IF NOT EXISTS registry_migrations (
   version BIGINT PRIMARY KEY,
   applied_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS capabilities (
   id TEXT PRIMARY KEY,
   kind TEXT NOT NULL,
   name TEXT NOT NULL,
   version TEXT NOT NULL,
   scope_kind TEXT NOT NULL,
   scope_key TEXT NOT NULL,
   manifest_json TEXT NOT NULL,
   manifest_digest TEXT NOT NULL,
   source_digest TEXT,
   revision BIGINT NOT NULL,
   enabled BIGINT NOT NULL DEFAULT 1,
   created_at BIGINT NOT NULL,
   updated_at BIGINT NOT NULL,
   UNIQUE(kind, name, scope_kind, scope_key)
);
CREATE INDEX IF NOT EXISTS idx_capabilities_lookup
   ON capabilities(kind, scope_kind, scope_key, enabled);
CREATE TABLE IF NOT EXISTS capability_versions (
   capability_id TEXT NOT NULL REFERENCES capabilities(id) ON DELETE CASCADE,
   revision BIGINT NOT NULL,
   version TEXT NOT NULL,
   manifest_json TEXT NOT NULL,
   manifest_digest TEXT NOT NULL,
   source_digest TEXT,
   created_at BIGINT NOT NULL,
   PRIMARY KEY(capability_id, revision)
);
CREATE TABLE IF NOT EXISTS capability_permissions (
   capability_id TEXT NOT NULL REFERENCES capabilities(id) ON DELETE CASCADE,
   permission TEXT NOT NULL,
   decision TEXT NOT NULL DEFAULT 'pending',
   decided_by TEXT,
   decided_at BIGINT,
   PRIMARY KEY(capability_id, permission)
);
CREATE TABLE IF NOT EXISTS capability_health (
   capability_id TEXT PRIMARY KEY REFERENCES capabilities(id) ON DELETE CASCADE,
   status TEXT NOT NULL,
   message TEXT,
   checked_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS registry_audit (
   id BIGSERIAL PRIMARY KEY,
   actor TEXT NOT NULL,
   action TEXT NOT NULL,
   capability_id TEXT,
   details_json TEXT NOT NULL,
   created_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_registry_audit_capability
   ON registry_audit(capability_id, id);
INSERT INTO registry_migrations(version, applied_at)
   VALUES(1, (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT)
   ON CONFLICT DO NOTHING;
