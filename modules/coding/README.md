# Coding

English | [简体中文](README.zh-CN.md)

`exoharness-coding` provides path, search, precise-edit and patch algorithms for multi-file coding tasks. Sandbox handles file access, Tools exposes invocation, and Artifacts publishes projects.

## Algorithms

- [paths.rs](src/paths.rs): `WorkspacePath::parse` validates workspace-relative paths.
- [search.rs](src/search.rs): `matches` searches text in supplied files and returns paths, line numbers and match summaries.
- [edits.rs](src/edits.rs): `Document` and `ExactEdit` describe input; `apply_edits` matches source text to prepare changes.
- [patch.rs](src/patch.rs): `parse` reads a patch, `affected` lists touched files, and `prepare` validates and creates candidate changes.

Algorithms prepare candidates from source text. Sandbox rechecks the workspace revision before committing changes; ambiguous matches and stale revisions return conflicts. Build/check runs in the same task workspace. On failure, files are retained so a follow-up edit can repair the relevant location.

## Tests

[tests/](tests/) covers paths and text processing, exact edits, multi-file patches and focused search using in-memory input.

```sh
cargo test --locked -p exoharness-coding
```

See [Sandbox](../sandbox/README.md) for file commits and [Coding workflow](../../docs/CODING_WORKFLOW.md) for the complete task flow.
