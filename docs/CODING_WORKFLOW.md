# Coding workflow

English | [简体中文](CODING_WORKFLOW.zh-CN.md)

Coding provides a workspace for code tasks, focused search, local edits, checks and delivery. Website generation is one use case. Language, business and visual conventions belong in on-demand Skills. Coding does not create a second Agent loop or bypass model, authorization, budgets or journals.

## Workflow

Create or reuse a task workspace → search/read a focused area → write files or apply an atomic patch → run a check against saved files → repair only the diagnosed area → verify the new revision → publish selected files.

Coding schemas are loaded when needed. Work files persist so a failed check can be repaired without recreating the whole project.

| Tool | Main inputs | Semantics |
| --- | --- | --- |
| `coding_search` | `workspace_id`, `query` | Returns relevant excerpts, line numbers, revision, SHA-256 and pagination |
| `coding_apply_patch` | `workspace_id`, `expected_revision`, `patch` | Multi-file Add/Update/Move/Delete in one compare-and-swap transaction |
| `coding_check` | workspace, revision, name, runtime, entrypoint | Runs a saved check in the sandbox; inline code cannot overwrite source |
| `coding_publish` | workspace, revision, files | Creates a project ZIP and optionally a static HTML preview |

General `workspace_*` and `sandbox_run` tools remain available for single files, documents, plots and similar work; they do not force every task into a website project.

## Edits and failures

Paths are relative to the workspace. Traversal, absolute paths and escaping symlinks are rejected. Patch anchors must be unambiguous; stale revisions, ambiguity or partial failure reject the entire patch. Replaying an idempotent operation does not modify already committed files again.

When a check exits unsuccessfully, source files and logs are retained so the failed part can be corrected. Checking and publishing are separate operations; publishing reads the saved files. Permission denial, storage conflicts, cancellation and unknown outcomes have distinct states.

Execution uses network-disabled fixed images, read-only package mounts and resource limits by default. Missing dependencies yield a diagnostic; there is no automatic network install or host-shell fallback. The complete legacy Task/Worker/Worktree/Review/Merge collaboration flow has not been migrated into Coding; old CLI commands do not apply.

## Delivery

The final publication manifest selects downloadable files; unselected intermediate files remain private. Multi-page static previews retain relative CSS, JavaScript and media resources and expose only manifest entries. Preview is isolated by CSP and does not run a backend service.

Projects support up to 128 files, 8 MiB per file, 32 MiB total and 4 MiB per patch. A model's single-response output limit depends on model configuration and the upstream API.

Implementation: `modules/coding` contains pure policies; `sandbox/src/task/coding.rs` commits workspace changes; `tools/src/coding*.rs` defines schemas and dispatch; `artifacts/src/project.rs` publishes projects; `api/src/media/project.rs` serves previews.

Acceptance covers multi-file projects, focused repair after failed checks, Unicode, revision conflicts, cancellation, ZIP contents and desktop/mobile resource loading.
