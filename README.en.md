# ExoHarness

[简体中文](README.md) | English · [Live demo](https://agent.clhh.tech/) · [Documentation](docs/README.md)

ExoHarness is a Rust-based Agent execution platform with multi-user chat, configurable assistants, Skill and MCP integration, isolated code execution, and unified file delivery. PostgreSQL persists conversations, execution records, and knowledge data. The management console provides access to model requests, tool arguments, execution events, and generated files.

## Try it and join the community

Visit the [live demo](https://agent.clhh.tech/) and register or sign in to use authorized assistants and capabilities. Avoid uploading sensitive information to the public demo.

Scan the QR code to join the ExoHarness WeChat technical discussion group. Its expiry date is shown in the image; the image will be updated when needed.

<img src="docs/assets/wechat-group.jpg" alt="ExoHarness WeChat community QR code" width="300" />

## Features

- Management console for models, assistants, capabilities, users, knowledge, execution traces, and audits.
- Streaming chat, searchable conversation history, voice input through an ASR service, and file preview/download.
- Authorized Skill and MCP integration with live JSON Schema validation and separate credential storage.
- Persisted Sessions, Turns, Runs, invocation logs, execution snapshots, bounded argument repair, progress, and cancellation.
- Multi-file Coding workflows with search, atomic patches, build/test checks, project ZIP delivery, and static site previews.
- OCI-isolated Python, Shell, JavaScript, and TypeScript execution with resource limits and reusable workspaces.
- Unified image, audio, video, and document delivery with provenance, SHA-256, signed access, and Range playback.
- Chinese and English interfaces in both the management console and user portal.

Specific document, slide, image, music, video, and teaching workflows depend on the capabilities and external services configured by the operator. See the [current implementation status](docs/STATUS.md) for supported features and remaining limitations.

## Repository scope

This repository contains the core framework, frontend, tests, build/deployment scripts, and documentation. Standalone packages in the root `skills/` and `mcp/` directories are maintained separately and are not distributed here. The integration modules `modules/skills` and `modules/mcp` remain part of the framework. Import the capabilities you need through the management console after deployment.

## Build and deploy

The current build scripts target Linux / WSL2. Install Rust/Cargo 1.88+, Python 3.11+, Node.js 22+, and the native build dependencies described in the [Chinese README](README.md#环境要求).

```sh
git clone git@github.com:jason571/Exoharness.git
cd Exoharness
cargo fetch --locked
npm --prefix web-console ci
bash scripts/package-release.sh
```

Native compilation does not require Docker. PostgreSQL is required at runtime. Docker is required for isolated code execution and can also host PostgreSQL or the main service. See the [build and deployment instructions](README.md#从源码编译) for database preparation, sandbox image setup, native/container startup, and private configuration.

Default local listeners are management `127.0.0.1:9797`, portal `127.0.0.1:9798`, and media `127.0.0.1:9790`. They are configurable. Public deployments should use an HTTPS reverse proxy and follow the [security guidance](docs/SECURITY.md).

## Documentation

The full technical documentation is currently in Chinese. English documentation will be added later; this page provides the English entry point in the meantime.

- [Architecture](docs/ARCHITECTURE.md), [API](docs/INTERFACES.md), [Database](docs/DATABASE.md)
- [Context and Token management](docs/CONTEXT_AND_TOKENS.md), [Capabilities](docs/CAPABILITIES.md), [Coding](docs/CODING_WORKFLOW.md)
- [Deployment](docs/DEPLOYMENT.md), [Security](docs/SECURITY.md), [Testing](docs/TESTING.md)
- [Documentation index](docs/README.md), [Contributing](CONTRIBUTING.md)

## License

Core code is licensed under the [MIT License](LICENSE). External Skills, MCP services, runtime images, model weights, and third-party dependencies retain their own licenses.
