# Open-source release checklist

English | [简体中文](OPEN_SOURCE_CHECKLIST.zh-CN.md)

Public releases use a reviewed source and asset manifest. The release owner records these checks together with the build version, test results and license inventory.

## Private content

- Exclude `.env`, private configuration generations, credentials, Secret Store data, database exports, user materials, task/artifact files, backups, real Trace data and logs.
- Inspect `mcp/*.json` and subproject configurations for real Authorization/API keys. Publish placeholder templates only; never expose usable headers.
- Exclude `target`, `node_modules`, caches, user data in screenshots, and local large images or model weights.
- Do not publish real users, sessions, lyrics, songs, images or signed URLs from test fixtures. Replace production replays with synthetic or redacted fixtures.
- Inspect Git history too. Rotate any credential that was ever public; deleting the current file does not remove historical exposure.

Scan the staging area and release archive independently for credentials and verify their file manifests. Review screenshots, demonstrations and logs as user data.

## Licensing

Core code is offered under AGPL-3.0-only, with an alternative commercial license for organizations that need to use it without complying with AGPL obligations. Copyright and licensing contact: Jason Yang, `flyangqn@gmail.com`. See the root `LICENSE` and `COMMERCIAL-LICENSE.md`. Third-party dependencies and resources keep their own licenses.

Review the licenses for each included Skill, MCP, font, image, frontend library and runtime image. Preserve their `LICENSE`/`NOTICE` files. Third-party capabilities do not automatically inherit the core license; model weights, service APIs and generated content also have separate terms.

Retain any license, notice and attribution required for redistributed resources. Obtain permission before including resources with unclear terms. Root-level `skills/` and `mcp/` are maintained as separate capability projects and are not included in this repository; the framework modules `modules/skills` and `modules/mcp` are part of the core release. Technical documentation is available in English by default, with Chinese versions linked from each document.

## Repository and version

1. Review `.gitignore` and packaging exclusions; verify private directories do not enter the staging area.
2. Keep lockfiles, required source/tests, placeholder examples, documentation and build/start scripts.
3. Reproduce a native build in a clean environment; inspect the release manifest and ABI without relying on private developer paths.
4. Run unit, database, protocol, OCI and browser gates, and identify real-model scenarios not covered.
5. Establish a private security-reporting channel and contribution rules; publish accurate limitations with each release.
6. Confirm contribution terms and produce a version, release record and public distribution manifest.

Current limitations are listed in [Feature status](STATUS.md). Contribution rules are in [CONTRIBUTING](../CONTRIBUTING.md).
