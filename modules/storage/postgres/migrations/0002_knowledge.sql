-- Preserve the original ExoHarness knowledge layout, including library scopes.
CREATE TABLE IF NOT EXISTS knowledge_documents (
 id TEXT PRIMARY KEY, owner_user_id TEXT NOT NULL, title TEXT NOT NULL,
 content TEXT NOT NULL, sha256 TEXT NOT NULL, source_kind TEXT NOT NULL,
 source_uri TEXT, revision BIGINT NOT NULL, created_at BIGINT NOT NULL,
 updated_at BIGINT NOT NULL
);
ALTER TABLE knowledge_documents ADD COLUMN IF NOT EXISTS category_id TEXT;
ALTER TABLE knowledge_documents ADD COLUMN IF NOT EXISTS tags_json TEXT NOT NULL DEFAULT '[]';
CREATE INDEX IF NOT EXISTS knowledge_documents_owner ON knowledge_documents(owner_user_id, updated_at DESC);
CREATE TABLE IF NOT EXISTS personal_memories (
 id TEXT PRIMARY KEY, owner_user_id TEXT NOT NULL, content TEXT NOT NULL,
 enabled BOOLEAN NOT NULL, source TEXT NOT NULL, revision BIGINT NOT NULL,
 created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL
);
CREATE INDEX IF NOT EXISTS personal_memories_owner ON personal_memories(owner_user_id, updated_at DESC);
CREATE TABLE IF NOT EXISTS knowledge_libraries (
 id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL,
 revision BIGINT NOT NULL, created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL
);
CREATE TABLE IF NOT EXISTS knowledge_library_grants (
 library_id TEXT NOT NULL REFERENCES knowledge_libraries(id) ON DELETE CASCADE,
 assistant_id TEXT NOT NULL, created_at BIGINT NOT NULL,
 PRIMARY KEY(library_id,assistant_id)
);
CREATE TABLE IF NOT EXISTS knowledge_categories (
 id TEXT PRIMARY KEY, scope_owner TEXT NOT NULL, name TEXT NOT NULL,
 parent_id TEXT, path TEXT NOT NULL, revision BIGINT NOT NULL,
 created_at BIGINT NOT NULL, updated_at BIGINT NOT NULL,
 UNIQUE(scope_owner,path)
);
CREATE INDEX IF NOT EXISTS knowledge_categories_owner ON knowledge_categories(scope_owner,path);
