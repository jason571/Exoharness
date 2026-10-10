//! On-demand engineering tools; inherit the existing sandbox grant.
use exoharness_contracts::model::Tool;
use serde_json::{Value, json};
fn tool(name: &str, description: &str, mut properties: Value, required: &[&str]) -> Tool {
    properties["workspace_id"] = json!({"type":"string","pattern":"^[a-f0-9]{64}$"});
    Tool {
        name: name.into(),
        description: description.into(),
        input_schema: json!({"type":"object","properties":properties,"required":required,"additionalProperties":false}),
    }
}
pub(crate) fn tools() -> Vec<Tool> {
    vec![
        tool(
            "coding_search",
            "Search related source across a private saved project without loading the entire repository. Literal query; bounded line excerpts with file SHA-256. Follow next_cursor and revision; skipped large/binary files are explicitly listed, never claimed searched. prefix restricts a file or subtree. Read focused full context with workspace_read before editing. No code executes.",
            json!({"query":{"type":"string","minLength":1,"maxLength":1000},"prefix":{"type":"string"},"file_offset":{"type":"integer","minimum":0},"line_offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":100}}),
            &["workspace_id", "query"],
        ),
        tool(
            "coding_apply_patch",
            "Atomically edit multiple project files using Codex-style *** Begin Patch / *** Add File: path / *** Update File: path / *** Delete File: path / *** End Patch text. Add lines start with +. Update hunks use @@, space-prefixed exact context, -old and +new lines; optional @@ exact unique context line, *** Move to: path, and *** End of File. Exact matching only, no fuzzy overwrite. All changes commit together at expected_revision or none do. Read-only Skill package and paths outside the task cannot be changed. Never execute a patch as a shell command.",
            json!({"expected_revision":{"type":"integer","minimum":1},"patch":{"type":"string","minLength":1}}),
            &["workspace_id", "expected_revision", "patch"],
        ),
        tool(
            "coding_check",
            "Run a saved build/test/check entrypoint ONCE in the same network-disabled OCI sandbox, preserving project files and diagnostic logs. Failed checks return actionable check.status=failed (not final delivery) so fix only faulty source with coding_apply_patch/workspace_edit, then check the NEW revision. No inline code, package installation or host fallback. Missing dependencies are real diagnostics, not permission to install. Timeout/cancellation/unknown outcome stops; never resubmit an unknown job. A passed build does NOT prove browser/UI acceptance. Use an authorized browser tool or sandbox-installed browser for that check. Does not publish intermediate files or satisfy final delivery; use coding_publish/workspace_publish after checks.",
            json!({"expected_revision":{"type":"integer","minimum":1},"name":{"type":"string","minLength":1,"maxLength":100},"runtime":{"type":"string","enum":["python","shell","javascript","typescript"]},"entrypoint":{"type":"string","minLength":1},"arguments":{"type":"array","items":{"type":"string"},"maxItems":128},"timeout_seconds":{"type":"integer","minimum":1,"maximum":3600}}),
            &[
                "workspace_id",
                "expected_revision",
                "name",
                "runtime",
                "entrypoint",
            ],
        ),
        tool(
            "coding_publish",
            "Publish selected saved project source files as one immutable ZIP preserving directories. Provide expected_revision and files. Optionally provide an HTML entrypoint to ALSO create a signed static preview with relative CSS/JS/images intact; omit entrypoint for ordinary code projects. Only selected files are published; no arbitrary host paths, directory listing or code execution. Backend services need separate authorized deployment and are not started here. Existing single-file/document workspace_publish remains available.",
            json!({"expected_revision":{"type":"integer","minimum":1},"entrypoint":{"type":"string","minLength":1},"files":{"type":"array","minItems":1,"maxItems":128,"uniqueItems":true,"items":{"type":"string"}}}),
            &["workspace_id", "expected_revision", "files"],
        ),
    ]
}
