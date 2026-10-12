# Context and Token management

English | [简体中文](CONTEXT_AND_TOKENS.zh-CN.md)

The execution journal stores complete facts. Model requests receive a projection focused on the current step. The goal is to remove repeated instructions, historical receipts and irrelevant schemas without changing creative quality, business constraints or source work data.

## Two data views

The journal stores inputs, model responses, original tool arguments, exact receipts, files and transformation evidence. The model window is rebuilt from that journal; compacting the window does not overwrite the journal. Structured arguments retain JSON types and are never damaged by summaries or character truncation of code, arrays or tables.

Ordinary text history selects complete user/assistant pairs, newest first. The current implementation reads at most eight turns within an approximately 32 KiB history-selection boundary and does not cut a message in half. Tool history must retain call/result pairs; it cannot leave an unmatched `tool_call_id`.

Source entry points: `modules/core/src/context.rs`, `modules/agent/src/{context,projection,workflow,catalog_context}.rs`.

## On-demand capabilities and Skills

The initial prompt provides general discovery, read and activation entry points, plus a paginated summary of authorized services. The model selects a relevant capability, queries that service's tools, then calls `activate_tools` to load live schemas required by the current stage. Service summaries include names, descriptions, versions and tool counts, not parameter schemas.

`search_capabilities` supports three query levels:

- `kind=capability`: find authorized execution services. An empty tool search without a service scope also returns this level.
- `kind=tool` with `capability_id`: search tool summaries within the selected service.
- `kind=tool` with `names`: batch-match exact native names or activation names declared by a Skill and report missing entries. It does not depend on keyword ranking.

`kind=skill` remains for Skill-instruction discovery. If a tool name has multiple authorized providers, the response retains each provider identity; activation uses exact names, and a native name activates only when the match is unique. Discovery, loading and execution are separate operations; none expands authorization or runs a tool automatically.

Service-index IDs define a search scope; they are not tool-loading names. If the model passes an authorized multi-tool service ID or an ambiguous duplicate tool to the loader, the loaded set remains unchanged and the response gives exact search guidance through the existing bounded correction budget. Unknown or unauthorized identities still stop at the permission boundary rather than loading a whole service.

If a model omits the public identity suffix from a tool name, the framework constructs a precise correction suggestion from the fixed authorized catalog. Resolution rules for full identity and native names remain unchanged; a partial name is never directly executable. A list of partial names is corrected as one complete list, preserving add/replace mode; no partial activation occurs. Ambiguous providers or normalized names require an exact lookup. Unknown names, typos and invalid full IDs are not guessed. One response supplies all corrections for a list and reuses the Run's existing bounded repair budget.

Each Skill revision's root instructions have one canonical copy. Directory lookups, reference files and root instructions are tracked separately and deduplicated by content identity. Discovery pagination passes the query and bound cursor through `next_page`; changing the query, service scope, name set or pinned revision restarts at page one. The old `offset` form remains valid within an unchanged query; an offset is not reused after query conditions change.

After successful activation, preceding catalog queries in model context are replaced by compact selection receipts, retaining the identity of selected tools. Full discovery pages and original calls remain in the journal; new stages can discover additional tools. Incomplete selections, failures, newer queries and error receipts remain visible. Business parameters, storyboards, code and MCP results do not use catalog-compaction rules.

Catalog summaries support discovery; live schemas, selected Skill execution instructions and user requirements remain available for execution and validation.

## Persistent checkpoints

After the complete call receipt is committed to the journal, the framework accepts a checkpoint explicitly declared by the service. It is based on protocol metadata returned by the tool and includes a locatable persistent reference.

The MCP extension uses `_meta.exoharness`:

```json
{
  "context_checkpoint": "opaque-checkpoint-reference",
  "context_preserve": ["/status", "/checkpoint"],
  "workflow_continuation": {
    "version": 1,
    "context_scope": "checkpoint",
    "status": "required",
    "checkpoint": {"id": "opaque-checkpoint-reference"},
    "required_tool": "continue_operation",
    "required_arguments": {"checkpoint_id": "opaque-checkpoint-reference"}
  }
}
```

This structure declares checkpoint metadata. Pointers resolve to actual values in the result or `structuredContent`; the next tool is resolved uniquely from the fixed authorized catalog and its arguments are checked against the live schema. `prerequisite_tools` declares prerequisites, which remain subject to existing authorization.

The valid projection replaces persisted arguments/results with compact references and preserves declared JSON values as-is. A stage request loads only the next tool schema and required prerequisites; rules from the active workflow Skill remain in context. After completion, delivery facts remain while consumed process data is not resent. A completed sub-workflow cannot clear an independent unfinished workflow.

If a field is missing, a pointer is invalid, permission is insufficient or the tool name is ambiguous, the framework falls back to a safe view; it does not guess data or turn a valid call into a failure. An explicit new user message or tool reactivation restores the corresponding ordinary view. Implementation entry points: `api/src/chat/checkpoint.rs`, `tools/src/workflow.rs`, `agent/src/workflow.rs`.

## Argument errors and stopping

Live JSON Schema validation checks objects, arrays, required fields, enums, ranges and combined constraints before submission. Clearly valid encoded JSON containers may have their local type restored; the framework does not guess missing fields or business defaults. Syntax repair, schema validation, authorization and actual execution are distinct facts.

Local repair retains the candidate and diagnostics and uses the Run-level budget. The default repeated-call limit is three consecutive calls with the same tool and arguments. Queue waits and background-state reads are handled by the execution backend and counted separately from model requests and generation submissions.

The repeat-control cursor stores a canonical native-JSON SHA-256 fingerprint,
with tool name and count. Original arguments remain in the durable journal.
Restoring or saving a legacy cursor preserves its accumulated repeat count;
the cursor does not duplicate large authored source files.

Current tool-chain budgets are 24 model requests, 48 tool calls, 300,000 reported input/output Tokens and 1 MiB of model-context bytes. The per-response output limit comes from model configuration and defaults to 4,096. Budgets protect execution; task cost is calculated from actual usage. Resuming or stopping with an unknown outcome preserves existing counters. Unified global cost scheduling remains pending.

## Progress and attachments

MCP progress and explicit background-task polling become UI events; polling is handled by the execution backend. Ordinary model requests receive attachment references and metadata. Binary files remain in separate storage.

Standard inline MCP image and audio blocks are retained in the original tool receipt. In the current text-only request contract, their Base64 payloads are projected as references containing the invocation ID, content index, MIME type and encoded size. Business JSON, authored text, resource links and call/result identity remain intact; the original receipt is not rewritten. This applies to ordinary exchanges and checkpoint views.

A media reference is not visual or audio input. Delivery uses the existing file link; inspection requires an authorized media-capable tool. Retrieving an inline result does not justify regenerating a completed file. File cards from multiple receipts are deduplicated by stable artifact identity, with URL and filename as a fallback.

## Accounting

Provider usage is the source of actual Token counts; input, output and cache fields are recorded separately. Cached input is part of input and is not added again to total Tokens. Failed requests count when the provider returns usage; otherwise usage is marked unknown rather than reported as zero.

Context bytes, estimated Tokens, schema count and cache ratio help identify redundancy but are not billing records. Session totals include multiple turns and Runs, so they are not the cost of one Run. Trace analysis should include model-request count, each request's input/output, repair count and actual capability calls.

Evaluate changes using comparable task types, models, inputs, successful delivery and output quality, recording process and usage breakdowns. No fixed savings percentage is promised. See [Feature status](STATUS.md) for the actual scope of safe HTTP Resume.
