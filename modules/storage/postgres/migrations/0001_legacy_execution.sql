CREATE TABLE IF NOT EXISTS execution_migrations (
                version BIGINT PRIMARY KEY,
                applied_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_sessions (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
                archived BIGINT NOT NULL DEFAULT 0,
                pinned BIGINT NOT NULL DEFAULT 0,
                owner_user_id TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_execution_sessions_updated
                ON execution_sessions(archived, updated_at DESC);
             CREATE TABLE IF NOT EXISTS execution_turns (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                ordinal BIGINT NOT NULL,
                prompt TEXT NOT NULL,
                history_content TEXT NOT NULL,
                selected_model TEXT,
                attachments_json TEXT NOT NULL,
                capability_ids_json TEXT NOT NULL,
                status TEXT NOT NULL,
                answer TEXT,
                response_json TEXT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
                UNIQUE(session_id, ordinal)
             );
             CREATE INDEX IF NOT EXISTS idx_execution_turns_session
                ON execution_turns(session_id, ordinal);
             CREATE TABLE IF NOT EXISTS execution_runs (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                turn_id TEXT NOT NULL REFERENCES execution_turns(id) ON DELETE CASCADE,
                attempt BIGINT NOT NULL,
                status TEXT NOT NULL,
                actor TEXT NOT NULL,
                error TEXT,
                cancel_requested BIGINT NOT NULL DEFAULT 0,
                created_at BIGINT NOT NULL,
                started_at BIGINT,
                finished_at BIGINT,
                last_sequence BIGINT NOT NULL DEFAULT 0,
                model_snapshot_json TEXT,
                capability_snapshot_json TEXT,
                UNIQUE(turn_id, attempt)
             );
             CREATE INDEX IF NOT EXISTS idx_execution_runs_session
                ON execution_runs(session_id, created_at DESC);
             CREATE INDEX IF NOT EXISTS idx_execution_runs_created ON execution_runs(created_at);
             CREATE INDEX IF NOT EXISTS idx_execution_runs_active ON execution_runs(status, created_at)
                WHERE status NOT IN ('succeeded','failed','cancelled','recoverable');
             CREATE TABLE IF NOT EXISTS execution_response_actions (
                sequence BIGSERIAL PRIMARY KEY,
                event_id TEXT NOT NULL,
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                actor TEXT NOT NULL,
                action TEXT NOT NULL CHECK(action IN ('copy','like','dislike','clear_feedback','regenerate')),
                reason TEXT,
                detail TEXT NOT NULL DEFAULT '',
                result_run_id TEXT REFERENCES execution_runs(id) ON DELETE CASCADE,
                created_at BIGINT NOT NULL,
                UNIQUE(actor, event_id),
                CHECK(char_length(detail) <= 200)
             );
             CREATE INDEX IF NOT EXISTS idx_response_actions_run_actor
                ON execution_response_actions(run_id, actor, sequence DESC);
             CREATE TABLE IF NOT EXISTS execution_regenerations (
                turn_id TEXT PRIMARY KEY REFERENCES execution_turns(id) ON DELETE CASCADE,
                source_run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                history_before_ordinal BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_capability_routes (
                turn_id TEXT PRIMARY KEY REFERENCES execution_turns(id) ON DELETE CASCADE,
                catalog_json TEXT NOT NULL,
                decision_json TEXT
             );
             CREATE TABLE IF NOT EXISTS execution_events (
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                sequence BIGINT NOT NULL,
                event_type TEXT NOT NULL,
                payload_json TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                PRIMARY KEY(run_id, sequence)
             );
             CREATE TABLE IF NOT EXISTS execution_context_checkpoints (
                run_id TEXT PRIMARY KEY REFERENCES execution_runs(id) ON DELETE CASCADE,
                context_json TEXT NOT NULL,
                updated_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_model_calls (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                logical_call_id TEXT NOT NULL,
                sequence BIGINT NOT NULL,
                call_kind TEXT NOT NULL,
                attempt BIGINT NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                api TEXT NOT NULL,
                status TEXT NOT NULL,
                stop_reason TEXT NOT NULL,
                error TEXT,
                response_id TEXT,
                started_at BIGINT NOT NULL,
                first_event_at BIGINT,
                completed_at BIGINT NOT NULL,
                duration_ms BIGINT NOT NULL,
                time_to_first_event_ms BIGINT,
                usage_reported BIGINT NOT NULL,
                usage_json TEXT NOT NULL,
                context_json TEXT NOT NULL,
                output_json TEXT NOT NULL,
                UNIQUE(run_id, sequence)
             );
             CREATE TABLE IF NOT EXISTS execution_request_parts (
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                sha256 TEXT NOT NULL,
                content_json TEXT NOT NULL,
                PRIMARY KEY(run_id,sha256)
             );
             ALTER TABLE execution_model_calls ADD COLUMN IF NOT EXISTS observation_sha256 TEXT;
             ALTER TABLE execution_model_calls ADD COLUMN IF NOT EXISTS accounting_version BIGINT NOT NULL DEFAULT 0;
             CREATE TABLE IF NOT EXISTS execution_usage_baselines (
                run_id TEXT PRIMARY KEY REFERENCES execution_runs(id) ON DELETE CASCADE,
                usage_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_large_results (
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                sha256 TEXT NOT NULL,
                result_json TEXT NOT NULL,
                PRIMARY KEY(run_id,sha256)
             );
             CREATE TABLE IF NOT EXISTS execution_model_requests (
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                logical_call_id TEXT NOT NULL,
                attempt BIGINT NOT NULL,
                sequence BIGINT NOT NULL,
                manifest_json TEXT NOT NULL,
                PRIMARY KEY(run_id,logical_call_id,attempt),
                UNIQUE(run_id,sequence)
             );
             CREATE TABLE IF NOT EXISTS execution_context_transforms (
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                sequence BIGINT NOT NULL,
                transform_json TEXT NOT NULL,
                PRIMARY KEY(run_id,sequence)
             );
             CREATE INDEX IF NOT EXISTS idx_execution_model_calls_run
                ON execution_model_calls(run_id, sequence);
             CREATE INDEX IF NOT EXISTS idx_execution_model_calls_started ON execution_model_calls(started_at);
             CREATE INDEX IF NOT EXISTS idx_execution_model_calls_logical
                ON execution_model_calls(run_id, logical_call_id, attempt);
             CREATE TABLE IF NOT EXISTS execution_invocations (
                id TEXT PRIMARY KEY,
                turn_id TEXT NOT NULL REFERENCES execution_turns(id) ON DELETE CASCADE,
                first_run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                latest_run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                capability_id TEXT NOT NULL,
                capability_revision BIGINT NOT NULL,
                tool_name TEXT NOT NULL,
                arguments_digest TEXT NOT NULL,
                arguments_json TEXT NOT NULL DEFAULT '{}',
                occurrence BIGINT NOT NULL,
                status TEXT NOT NULL,
                outcome_status TEXT,
                result_json TEXT,
                result_digest TEXT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL,
                submitted_at BIGINT,
                completed_at BIGINT,
                UNIQUE(turn_id, capability_id, capability_revision, tool_name, arguments_digest, occurrence)
             );
             CREATE INDEX IF NOT EXISTS idx_execution_invocations_turn
                ON execution_invocations(turn_id, created_at);
             CREATE INDEX IF NOT EXISTS idx_execution_invocations_run
                ON execution_invocations(latest_run_id, status);
             CREATE TABLE IF NOT EXISTS execution_background_observations (
                id TEXT PRIMARY KEY,
                sequence BIGSERIAL NOT NULL UNIQUE,
                invocation_id TEXT NOT NULL REFERENCES execution_invocations(id) ON DELETE CASCADE,
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                capability_revision BIGINT NOT NULL,
                result_json TEXT NOT NULL,
                result_digest TEXT NOT NULL,
                observed_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_execution_background_observations_invocation
                ON execution_background_observations(invocation_id, sequence);
             CREATE TABLE IF NOT EXISTS execution_managed_callback_runs (
                callback_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                tool_name TEXT NOT NULL,
                arguments_digest TEXT NOT NULL,
                run_id TEXT NOT NULL UNIQUE REFERENCES execution_runs(id) ON DELETE CASCADE,
                created_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_run_budgets (
                run_id TEXT PRIMARY KEY REFERENCES execution_runs(id) ON DELETE CASCADE,
                limits_json TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                stop_reason TEXT,
                created_at BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_run_artifacts (
                id TEXT PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES execution_runs(id) ON DELETE CASCADE,
                source_url TEXT NOT NULL,
                filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                created_at BIGINT NOT NULL,
                UNIQUE(run_id, source_url)
             );
             CREATE INDEX IF NOT EXISTS idx_execution_run_artifacts_run
                ON execution_run_artifacts(run_id, created_at);
             CREATE TABLE IF NOT EXISTS execution_artifact_files (
                artifact_id TEXT PRIMARY KEY REFERENCES execution_run_artifacts(id) ON DELETE CASCADE,
                relative_path TEXT NOT NULL,
                size_bytes BIGINT NOT NULL,
                sha256 TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_artifact_configuration (
                singleton_id BIGINT PRIMARY KEY CHECK(singleton_id=1),
                revision BIGINT NOT NULL,
                access_ttl_seconds BIGINT NOT NULL CHECK(access_ttl_seconds>=0),
                updated_at BIGINT NOT NULL
             );
             INSERT INTO execution_artifact_configuration VALUES(1, 1, 0, 0)
                ON CONFLICT DO NOTHING;
             CREATE TABLE IF NOT EXISTS execution_session_metadata (
                session_id TEXT PRIMARY KEY REFERENCES execution_sessions(id) ON DELETE CASCADE,
                managed_coding BIGINT NOT NULL DEFAULT 0,
                model TEXT,
                provider TEXT,
                system_prompt TEXT,
                cwd TEXT,
                parent_id TEXT REFERENCES execution_sessions(id) ON DELETE SET NULL,
                child_budget BIGINT NOT NULL DEFAULT 0,
                tagline TEXT,
                auto_archive BIGINT NOT NULL DEFAULT 0,
                notify_parent BIGINT NOT NULL DEFAULT 1,
                project_name TEXT,
                sandbox_profile TEXT,
                successor_id TEXT REFERENCES execution_sessions(id) ON DELETE SET NULL
             );
             CREATE TABLE IF NOT EXISTS execution_session_agent_profiles (
                session_id TEXT PRIMARY KEY REFERENCES execution_sessions(id) ON DELETE CASCADE,
                profile_snapshot_json TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS execution_run_agent_profiles (
                run_id TEXT PRIMARY KEY REFERENCES execution_runs(id) ON DELETE CASCADE,
                profile_snapshot_json TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_execution_session_parent
                ON execution_session_metadata(parent_id);
             CREATE TABLE IF NOT EXISTS execution_session_notices (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                kind TEXT NOT NULL,
                content TEXT NOT NULL,
                sender TEXT,
                created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_execution_notices_session
                ON execution_session_notices(session_id, created_at);
             CREATE TABLE IF NOT EXISTS execution_queued_chats (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                content TEXT NOT NULL,
                sender TEXT NOT NULL,
                created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_execution_queued_chats_session
                ON execution_queued_chats(session_id, created_at);
             CREATE TABLE IF NOT EXISTS execution_post_idle_actions (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES execution_sessions(id) ON DELETE CASCADE,
                action_json TEXT NOT NULL,
                created_at BIGINT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_execution_post_idle_session
                ON execution_post_idle_actions(session_id, created_at);
             CREATE TABLE IF NOT EXISTS execution_scheduler_configuration (
                singleton_id BIGINT PRIMARY KEY CHECK(singleton_id=1),
                max_active_runs_per_user BIGINT NOT NULL,
                max_global_agent_runs BIGINT NOT NULL,
                resource_pools_json TEXT NOT NULL,
                mcp_resource_assignments_json TEXT NOT NULL,
                revision BIGINT NOT NULL,
                updated_at BIGINT NOT NULL
             );
             INSERT INTO execution_scheduler_configuration(
                singleton_id, max_active_runs_per_user, max_global_agent_runs,
                resource_pools_json, mcp_resource_assignments_json, revision, updated_at
             ) VALUES(
                1, 2, 1000, '[]', '{}', 1,
                (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT
             ) ON CONFLICT DO NOTHING;
             INSERT INTO execution_migrations(version, applied_at)
                VALUES(1, (EXTRACT(EPOCH FROM clock_timestamp()) * 1000)::BIGINT)
                ON CONFLICT DO NOTHING;
             ALTER TABLE execution_sessions
                ADD COLUMN IF NOT EXISTS owner_user_id TEXT;
             ALTER TABLE execution_sessions
                ADD COLUMN IF NOT EXISTS pinned BIGINT NOT NULL DEFAULT 0;
             CREATE INDEX IF NOT EXISTS idx_execution_sessions_owner
                ON execution_sessions(owner_user_id, archived, updated_at DESC);
             ALTER TABLE execution_invocations
                ALTER COLUMN arguments_json SET DEFAULT '{}';
