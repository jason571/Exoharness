# GitHub publication

## Project metadata

Repository: `jason571/Exoharness` (public).

Description:

> Rust AI agent framework and self-hosted web console with MCP, Skills, sandboxed execution, persistent sessions, tracing, and file delivery.

Homepage: <https://agent.flyang.art/>

Topics: `ai-agent`, `agent-framework`, `agent-harness`, `rust`, `mcp`, `model-context-protocol`, `llm`, `self-hosted`, `sandbox`, `workflow-automation`.

GitHub language statistics exclude the six esbuild outputs declared in `.gitattributes`. Handwritten JavaScript and TypeScript sources remain counted. GitHub recalculates statistics asynchronously after a push.

## Prepare a public binary asset

```bash
./scripts/package-release.sh --skip-web --offline
python3 scripts/package-github-release.py --version v0.1.0
```

`release/github/` contains the archive and `SHA256SUMS`. The archiver verifies the build manifest and packages only its public file allowlist, including both startup documents and licensing files. It records the source commit, binary digest and minimum glibc symbol version. Keep production data and credentials outside this directory.

Validate the extracted archive against a disposable database and ports before publishing. Record only sanitized evidence in `release-validation.json`. The initial preview validation covers startup, configuration, persistence and file delivery; it does not establish model, GPU or OCI task success.

## Publish

Publishing requires a GitHub account with release write access; editing repository metadata requires the applicable repository management permission. SSH push access and the active `gh` API account may differ. Check `gh api user --jq .login` and repository permissions first.

Tag the verified source commit as `v0.1.0`, create a draft prerelease with [these release notes](releases/v0.1.0.md), attach the runnable archive, `SHA256SUMS` and `release-validation.json`, then publish. Confirm the public release and asset checksums afterward.
