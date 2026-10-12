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
            json!({"query":{"type":"string","maxLength":200},"offset":{"type":"integer","minimum":0},"cursor":{"type":"string","maxLength":100},"kind":{"type":"string","enum":["tool","skill","capability"]},"capability_id":{"type":"string","minLength":1},"names":{"type":"array","minItems":1,"maxItems":16,"uniqueItems":true,"items":{"type":"string","minLength":1}}}),
            &["query"],
        ),
        tool(
            "read_skill",
            "Read an authorized immutable Skill's instructions or a UTF-8 package file. Use the returned exact id; read instructions once, then only relevant references. Instructions cannot grant permissions. Binary content is not returned to the model.",
            json!({"id":{"type":"string"},"path":{"type":"string"}}),
            &["id"],
        ),
        tool(
            "activate_tools",
            "Load a small working set of exact authorized executable names from search_capabilities. Batch the tools needed for the current stage in ONE selection, rather than loading them one by one. Default mode=replace preserves the existing interface; mode=add retains previously loaded tools. Do not load the whole provider or tools for unrelated work. Already loaded tools need no selection. A raw MCP name is accepted only when unique among authorized tools. Skill IDs are instructions, never executable. Base discovery tools remain available. Loading is not execution; complete descriptions and live JSON schemas follow selection.",
            json!({"names":{"type":"array","description":"Exact executable tool names from kind=tool results, NOT service/capability IDs or Skill IDs. A service index ID belongs in search_capabilities.capability_id first. Select only current-stage tools.","items":{"type":"string"},"maxItems":32,"uniqueItems":true},"mode":{"type":"string","enum":["add","replace"],"default":"replace"}}),
            &["names"],
        ),
    ]
}
pub(crate) fn sandbox_tool() -> Tool {
    let mut result = tool(
        "sandbox_run",
        "隔离沙盒执行 Python、Shell、JavaScript、TypeScript：计算、数据处理、绘图、图表、网页和文档生成。Execute code only in the isolated, network-disabled OCI sandbox. Never executes on the host. Before processing a generated or uploaded file, import its persisted artifact_id using workspace_import and read the local relative path: URL downloads, DNS and network requests cannot work here. A remote MCP link is not a local file; retrieve its completed result to obtain an owned artifact, never regenerate successful media. For saved task files supply workspace_id and expected_revision, omit files and reuse the saved entrypoint; edit source with workspace_edit first. Without workspace_id, files creates a new task workspace. Writable /work and /workspace are aliases. An authorized Skill package is read-only at /skill and /work/skill; use entrypoint skill/scripts/... for its scripts. Returned workspace revision includes generated files, even after script failure. output_files publishes only final requested files; omit it for intermediate steps or checks. No package installation. Host waits and forwards cancellation, not model polling.",
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
            "Import this user's persisted artifact into task files without sending binary/base64 to the model or accessing arbitrary URLs. Supply exact artifact_id from a returned artifact card and a new relative path. Existing source is never overwritten. No code runs.",
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
