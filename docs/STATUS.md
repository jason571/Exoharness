# Current feature status

English | [简体中文](STATUS.zh-CN.md)

Status is listed by interface and execution path. Baseline tests, real tasks and deployment checks are recorded separately.

## Connected

- Three service endpoints, separate user/administrator authentication, model and assistant configuration, user assignments, registration email and personal settings.
- Streaming chat, conversation titles/search/pinning/history, response actions, Trace and audit records.
- Chinese/English switching and saved browser language preference on management and user portals.
- Skill import/read/revision management; remote HTTP MCP initialization, discovery and calls; Secret management.
- On-demand capabilities and live schemas, invocation ledger, pinned snapshots, bounded repair and repeated-call protection.
- Real OCI Python/Shell/JS/TS execution and native workspaces; Coding patches, checks and delivery.
- Actual MCP progress, explicit background-task polling/cancellation and persistent concurrency admission per MCP.
- Management scheduler settings, global/per-user Run admission, cross-MCP shared resource pools and live usage overview.
- Artifact storage, signed downloads, media playback and static project previews.
- Isolated DOCX/text-PDF import, categorization, structural block reads, literal search and personal-memory management.
- LLM protocols, Volcengine TTS SSE and compatible ASR execution; see the model document for adapter details.

## Remaining limitations

| Area | Current boundary |
| --- | --- |
| Safe Run Resume | HTTP resume currently returns `resume_unavailable`; event replay and checkpoint storage are available |
| Scheduler recovery | Settings and in-flight reservations persist; after a process failure, an external call with unknown outcome is not automatically resubmitted |
| User self-installation | Policy and catalog views exist; installation execution is not connected and returns `installation_ready=false` |
| stdio MCP | Can be registered but is not launched by the host; managed execution is incomplete |
| Standalone ToolNode | Record type exists, but standalone execution preparation is incomplete; native Sandbox has a separate execution path |
| Skill runtime building | Pinned image validation and native sandbox execution work; automatic per-package dependency downloads and arbitrary uv/Conda/OCI builds are unsupported |
| Sandbox inside main container | External execution proxy is pending; full code execution currently uses native main-service mode |
| Model types | Connected protocols execute through model adapters; new protocols need a matching adapter |
| Automatic knowledge use | Literal-search API works; automatic chat retrieval, pgvector/embeddings, OCR and semantic PDF table recognition are not implemented |
| Git collaboration | Multi-file Coding works; the full legacy Task/Worker/Worktree/Review/Merge workflow has not migrated |
| Dynamic plugins | Static dependencies/lifecycle work; hot replacement, full generation leases and dynamic code loading are unsupported |
| Public access/database TLS | Current origin checks assume local authentication and PostgreSQL uses NoTls; public access needs a matching proxy policy |
| Acceptance | Unit, protocol, database, OCI and browser tests exist; long-term multi-model success/cost tracking continues |

Some diagnostics retain `product_ready` and `infrastructure_validation` fields; their values describe the validation scope of those endpoints. Deployment acceptance is based on the actual execution path and artifact inspection.
