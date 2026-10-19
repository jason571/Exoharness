// Copyright (C) 2026 Jason Yang <flyangqn@gmail.com>
// SPDX-License-Identifier: AGPL-3.0-only
// Commercial licensing: contact flyangqn@gmail.com.

use exoharness_contracts::model::Tool;
use serde_json::json;
fn tool(name: &str, description: &str, properties: serde_json::Value, required: &[&str]) -> Tool {
    Tool {
        name: name.into(),
        description: description.into(),
        input_schema: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    }
}
pub(crate) fn base() -> Vec<Tool> {
    vec![
        tool(
            "search_capabilities",
            "Discover metadata, then load selected tools. Use kind=capability to choose an authorized service by task; then kind=tool with capability_id to inspect only that service. If a Skill supplies exact tool names, find them together with names (raw or activation names); no full-catalog scan is needed. kind=skill finds instructions. Empty unscoped queries return a service index, not every tool. Matching is lexical: a miss is not global unavailability. Use next_page arguments unchanged for pagination; changing query/scope starts a new search. Results have exact activation names but no parameter schemas. Does not execute or load tools.",
            json!({"query":{"type":"string","maxLength":200},"offset":{"type":"integer","minimum":0},"cursor":{"type":"string","maxLength":100},"kind":{"type":"string","enum":["tool","skill","capability"]},"capability_id":{"type":"string","minLength":1,"description":"Only with kind=tool: exact executable service ID returned by the authorized capability index. Omit for kind=skill or kind=capability. Never use a Skill ID here."},"names":{"type":"array","minItems":1,"maxItems":16,"uniqueItems":true,"items":{"type":"string","minLength":1}}}),
            &["query"],
        ),
        tool(
            "read_skill",
            "Read an authorized immutable Skill's instructions or a UTF-8 package file. Use the exact authorized id and read instructions once. path defaults to SKILL.md. Optional query selects resources only when path=resource-index.json; ordinary file reads return complete content even when query is present. For an indexed Skill at most two matching resources are loaded; no match loads none. Optional load_tools also loads known authorized executable names in this same call, retaining the loaded set. For native code/document authoring, request sandbox_run here when authorized, avoiding a separate search/activation round. No code executes and no permissions are granted. Unknown or denied selections fail atomically. Binary content is not returned to the model.",
            json!({"id":{"type":"string","description":"Exact Skill ID from authorized Skill metadata; never a service/tool ID."},"path":{"type":"string"},"query":{"type":"string","minLength":1,"maxLength":2000,"description":"Optional task hint. Selects related resources with path=resource-index.json. Ignored for complete root/reference file reads."},"load_tools":{"type":"array","maxItems":16,"uniqueItems":true,"items":{"type":"string","minLength":1},"description":"Optional exact already-known authorized executable names, e.g. sandbox_run for code authoring. Adds schemas only after a successful read; does not run tools or unload existing tools."}}),
            &["id"],
        ),
        tool(
            "activate_tools",
            "Load exact authorized executable names from discovery or host workspace guidance; known native names need no further search. Batch current-stage tools in one selection. Use mode=add to retain previously loaded entry tools when adding a dependency. Default mode=replace replaces the loaded set. Already loaded tools need no selection. Do not load unrelated tools or a whole provider. A raw MCP name must resolve uniquely. Skill IDs are never executable. Base discovery stays available. Loading only supplies descriptions and live JSON Schemas; it never executes.",
            json!({"names":{"type":"array","description":"Exact executable tool names from kind=tool results, NOT service/capability IDs or Skill IDs. A service index ID belongs in search_capabilities.capability_id first. Select only current-stage tools.","items":{"type":"string"},"maxItems":32,"uniqueItems":true},"mode":{"type":"string","enum":["add","replace"],"default":"replace"}}),
            &["names"],
        ),
    ]
}
pub(crate) fn sandbox_tool() -> Tool {
    let mut result = tool(
        "sandbox_run",
        "隔离沙盒执行 Python、Shell、JavaScript、TypeScript：计算、数据处理、绘图、图表、网页和文档生成。Execute code only in the isolated, network-disabled OCI sandbox. Never executes on the host. Before processing a generated or uploaded file, import its persisted artifact_id using workspace_import and read the local relative path: URL downloads, DNS and network requests cannot work here. Use a native artifact_id beginning artifact- from _host_saved_artifacts or _host_delivered_artifacts; a remote job_id or URL is not an artifact_id. Result retrieval may still return only a URL. In that case use an authorized service that accepts the existing URL, or report the missing handoff; never download URLs in code or regenerate successful media. For saved task files supply workspace_id and expected_revision, omit files and reuse the saved entrypoint; edit source with workspace_edit first. Without workspace_id, files creates a new task workspace. Writable /work and /workspace are aliases. An authorized Skill package is read-only at /skill and /work/skill; use entrypoint skill/scripts/... for its scripts. Returned workspace revision includes generated files, even after script failure. output_files publishes only final requested files; omit it for intermediate steps or checks. No package installation. Host waits and forwards cancellation, not model polling.",
        json!({
            "runtime":{"type":"string","enum":["python","shell","javascript","typescript"],"description":"Node .js/.mjs/.cjs uses javascript, never python or node. Python .py uses python, .sh uses shell, .ts/.mts/.cts uses typescript."},
            "entrypoint":{"type":"string","minLength":1,"description":"Safe saved relative path matching runtime. Packaged Skill scripts use skill/scripts/...; no absolute paths. Arguments refer to task files in the same saved workspace."},
            "files":{"type":"object","additionalProperties":{"type":"string"},"maxProperties":128},
            "skill_id":{"type":"string","description":"Only for creation with new files. Omit when workspace_id is present: the saved workspace already binds its immutable Skill revision."},
            "workspace_id":{"type":"string","pattern":"^[a-f0-9]{64}$"},
            "expected_revision":{"type":"integer","minimum":1},
            "timeout_seconds":{"type":"integer","minimum":1,"maximum":3600},
            "arguments":{"type":"array","items":{"type":"string"},"maxItems":128},
            "output_files":{"type":"array","items":{"type":"string"},"maxItems":32,"uniqueItems":true}
        }),
        &["runtime", "entrypoint", "output_files"],
    );
    // Model-visible alternatives must match the execution backend. A source
    // upload and an existing-workspace invocation are distinct operations.
    result.input_schema["oneOf"] = json!([
        {"title":"Run existing saved files","required":["workspace_id","expected_revision"],"properties":{"files":{"maxProperties":0}},"not":{"required":["skill_id"]}},
        {"title":"Create and run new files","required":["files"],"properties":{"files":{"minProperties":1}},"not":{"anyOf":[{"required":["workspace_id"]},{"required":["expected_revision"]}]}}
    ]);
    result
}
pub(crate) fn sandbox_tools() -> Vec<Tool> {
    let id = json!({"type":"string","pattern":"^[a-f0-9]{64}$"});
    let revision = json!({"type":"integer","minimum":1});
    let mut tools = vec![
        sandbox_tool(),
        tool(
            "workspace_import",
            "Import this user's persisted artifact into task files without sending binary/base64 to the model or accessing arbitrary URLs. Supply exact native artifact_id beginning artifact- from a host saved-file receipt (_host_saved_artifacts for internal files, _host_delivered_artifacts for delivered files) and a new relative path. Never supply an MCP job_id, remote descriptor ID or URL. A remote-only result needs an authorized URL-consuming service, not sandbox networking. Existing source is never overwritten. No code runs.",
            json!({"workspace_id":id,"expected_revision":revision,"inputs":{"type":"array","minItems":1,"maxItems":12,"items":{"type":"object","additionalProperties":false,"properties":{"artifact_id":{"type":"string"},"path":{"type":"string"}},"required":["artifact_id","path"]}}}),
            &["workspace_id", "expected_revision", "inputs"],
        ),
        tool(
            "workspace_list",
            "Recover saved workspace IDs/revisions for this user's CURRENT session. Use for follow-up edits instead of regenerating files. No file content, host paths or another user's workspaces are returned.",
            json!({}),
            &[],
        ),
        tool(
            "workspace_job",
            "Inspect a saved sandbox invocation or request cancellation of that job only. Use returned workspace_id and execution.invocation_id. Cancellation is confirmed only by a persisted receipt with cleanup_confirmed=true; a request alone is not completion. Host normally waits without model polling.",
            json!({"workspace_id":id,"invocation_id":{"type":"string"},"action":{"type":"string","enum":["status","cancel"]}}),
            &["workspace_id", "invocation_id", "action"],
        ),
        tool(
            "workspace_create",
            "Create a private persistent task workspace. No code executes. Reuse its exact ID/revision for source edits, sandbox_run, checks and final publication. Optional skill_id binds an authorized immutable package; ordinary calculations need no Skill.",
            json!({"skill_id":{"type":"string"}}),
            &[],
        ),
        tool(
            "workspace_read",
            "List saved file names, sizes and SHA-256 when path is omitted; otherwise read a bounded UTF-8 slice or find a literal query. Offsets count Unicode characters. Binary files are never encoded for the model. next_offset=null means exhausted; prefer focused queries over repeated full reads. Workspaces remain readable after failure.",
            json!({"workspace_id":id,"path":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":16000},"query":{"type":"string","minLength":1,"maxLength":1000}}),
            &["workspace_id"],
        ),
        tool(
            "workspace_edit",
            "Atomically write UTF-8 task files or patch only faulty portions at expected_revision. Patches require the current file SHA-256 and one unique before match; all matches refer to the SAME original file and must not overlap. Whole batch is rejected on conflict; no code executes. Skill package paths cannot be edited. Return keeps compact receipts, not source copies.",
            json!({"workspace_id":id,"expected_revision":revision,"files":{"type":"object","additionalProperties":{"type":"string"},"maxProperties":128},"patches":{"type":"array","maxItems":128,"items":{"type":"object","additionalProperties":false,"properties":{"path":{"type":"string"},"sha256":{"type":"string"},"before":{"type":"string","minLength":1},"after":{"type":"string"}},"required":["path","sha256","before","after"]}},"deletes":{"type":"array","maxItems":128,"uniqueItems":true,"items":{"type":"string"}},"notes":{"type":"string","maxLength":8000}}),
            &["workspace_id", "expected_revision"],
        ),
        tool(
            "workspace_publish",
            "Publish existing final task files from expected_revision without rerunning code. Only requested final files become download/preview artifacts. Intermediates remain private. Paths are relative to this task, never host paths or arbitrary URLs.",
            json!({"workspace_id":id,"expected_revision":revision,"output_files":{"type":"array","minItems":1,"maxItems":32,"uniqueItems":true,"items":{"type":"string"}}}),
            &["workspace_id", "expected_revision", "output_files"],
        ),
    ];
    tools.extend(crate::coding_schema::tools());
    tools
}

#[cfg(test)]
#[path = "../tests/unit/native_schema.rs"]
mod tests;
