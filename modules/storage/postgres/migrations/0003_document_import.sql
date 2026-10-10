-- Additive; plaintext records and legacy columns remain valid.
ALTER TABLE knowledge_documents ADD COLUMN IF NOT EXISTS extracted_json jsonb;
ALTER TABLE knowledge_documents ADD COLUMN IF NOT EXISTS source_file_json jsonb;
