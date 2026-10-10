-- Distinguish a placeholder from an explicitly named conversation. No model
-- requests and no changes to prompts, results, execution state or ownership.
ALTER TABLE execution_sessions ADD COLUMN IF NOT EXISTS title_source TEXT NOT NULL DEFAULT 'legacy';
UPDATE execution_sessions SET title_source='manual'
WHERE title_source='legacy' AND title NOT IN ('新对话','New conversation');
UPDATE execution_sessions s SET
    title=left(regexp_replace(trim(t.prompt),'[[:space:]]+',' ','g'),60)
        || CASE WHEN length(regexp_replace(trim(t.prompt),'[[:space:]]+',' ','g'))>60 THEN '…' ELSE '' END,
    title_source='auto'
FROM execution_turns t
WHERE s.id=t.session_id AND t.ordinal=1 AND s.title_source='legacy'
    AND s.title IN ('新对话','New conversation') AND length(trim(t.prompt))>0;
UPDATE execution_sessions SET title_source='placeholder'
WHERE title_source='legacy' AND title IN ('新对话','New conversation');
