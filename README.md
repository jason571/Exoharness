# ExoHarness

English | [简体中文](README.zh-CN.md) · [Live demo](https://agent.clhh.tech/) · [Documentation](docs/README.md)

**A self-hosted Agent platform for turning conversations into real work and deliverable files.**

ExoHarness connects language models, Skills, MCP services, and isolated code execution behind a multi-user web application. Users describe what they want; an authorized assistant discovers the relevant capabilities, calls tools, reports execution progress, and delivers the resulting documents, websites, images, audio, or videos in the conversation.

The platform is written in Rust. It provides the execution runtime, administration, access control, PostgreSQL persistence, and file delivery. The models and specialized services that perform creative or business tasks are configured separately.

## What can you use it for?

ExoHarness is useful when you want to provide several assistants through one application, connect your own tools and models, and inspect how a task was actually executed.

| Task | Example request | Deliverable and capabilities |
| --- | --- | --- |
| Research and data analysis | “Find recent US GDP figures and chart the trend.” | Search results, a chart, or a spreadsheet; uses authorized search tools and sandbox execution. |
| Office documents | “Create an entertainment industry report as a presentation.” | Downloadable PPTX, DOCX, or PDF; uses the corresponding Skills and document libraries in the sandbox. |
| Websites and coding | “Build a bilingual tourism website with a homepage and detail pages.” | Multi-file project, browser preview, and ZIP download; uses Coding and sandbox tools. |
| Images and posters | “Design a promotional poster for this company.” | Previewable image and download; uses an image-generation service and design instructions. |
| Music and video | “Make a narrated promotional video” or “Create an MV from this song.” | Audio/video files and a final combined output when the configured workflow supports it; uses generation and media-processing services. |
| Educational content | “Explain this question to a primary-school student with an animation.” | Narration and an explanatory video; uses a teaching Skill, TTS, and an animation renderer. |

These are examples of workflows enabled by configured capabilities, not models bundled with the core. The standalone `skills/` and `mcp/` projects are distributed separately. Import and authorize the required capabilities before using them.

## From a request to a result

1. An administrator configures model connections, imports capabilities, and assigns an assistant and permissions to users.
2. A user sends a request, optionally with reference files, through streaming chat.
3. The assistant discovers authorized capabilities and loads the tool schemas needed for the current step.
4. MCP services or isolated sandbox tools perform the work. Progress and execution records are persisted.
5. The user receives previews and downloads. Administrators can inspect the request, tool calls, errors, usage, and files in Trace.

## Core features

### Multi-user chat and assistant administration

The user portal includes streaming responses, searchable history, conversation management, attachments, voice input through ASR, personal materials and memory management, and file preview/download. Administrators configure assistant identity, avatars, model choices, and capability permissions, then assign assistants to users. Both interfaces support Chinese and English.

### Skills, MCP services, and model connections

Register versioned Skill packages and remote MCP services through the management console. Tool discovery follows the user's and assistant's authorization, enabled state, and runtime readiness. Execution arguments are validated against live JSON Schemas. Model adapters connect LLM, TTS, and ASR protocols; credentials are stored separately from public capability descriptions.

### Inspectable execution

Sessions, Turns, Runs, invocation records, snapshots, and events are persisted in PostgreSQL. Trace exposes model requests, tool arguments and results, errors, Token usage, and generated files. Progress from MCP services is forwarded to the interface; declared background tasks can be polled and cancelled by the execution backend.

### Context and Token management

The execution journal keeps complete inputs and results, while model requests use a task-focused projection. ExoHarness loads tool schemas on demand, deduplicates repeated Skill instructions, and replaces eligible persisted exchanges with compact checkpoint references. Argument repair and repeated-call guards have bounded budgets. Usage statistics distinguish input, output, and cache usage. See [Context and Token management](docs/CONTEXT_AND_TOKENS.md) for the actual rules and limits.

### Isolated execution and Coding

Python, Shell, JavaScript, and TypeScript tasks run in OCI containers with resource limits, read-only package mounts, writable task workspaces, and networking disabled by default. Files can be reused and edited within a task. The Coding module adds multi-file search, exact edits and atomic patches, build/test checks, project archives, and static website previews.

### Unified deliverables

One file service handles images, audio, video, office documents, and published static projects. It records provenance and SHA-256, provides signed access, and supports Range requests for media playback. Chat displays declared final artifacts as previews or download cards.

### Knowledge and operations

Manage personal or administrator-provided materials, categories, DOCX and text-based PDF imports, structured blocks, and literal search. The administration console also provides user management, execution overviews, audit records, Run admission limits, MCP concurrency, and shared resource pools. Automatic knowledge retrieval, embeddings, and OCR are not part of the current implementation.

## Architecture at a glance

```text
Management console :9797       User portal :9798
              \                  /
                  ExoHarness
                  ├── PostgreSQL: identities, conversations, execution records
                  ├── Configuration, capability registry, and secret storage
                  ├── OCI sandbox: code, scripts, and task workspaces
                  └── MCP services and model APIs: external capabilities

File service :9790 ── downloads, media playback, static site previews
```

The listeners bind to `127.0.0.1` by default and are configurable. Source modules live in `modules/`; Rust tests live in module `tests/` directories, with script and browser tests in `scripts/tests/`.

## Which environments are needed?

| Component | Purpose | When it is needed |
| --- | --- | --- |
| Rust / Cargo 1.88+ | Compile the core service. | Building from source; not needed to run a prebuilt binary. |
| Host Python 3.11+ | Run build, packaging, database preparation, startup, migration, and test scripts. | Using the supplied scripts. The core service itself is a Rust executable. |
| Host Node.js 22+ / npm | Type-check and bundle the TypeScript frontend. | Building the frontend; not needed to serve an already-built release. |
| PostgreSQL | Persist users, configuration records, conversations, execution, and knowledge data. | Running the platform. |
| Docker Engine | Run isolated task containers; optionally host PostgreSQL or the main service. | Sandbox execution or container deployment. Native core compilation does not require Docker. |
| Python / Node inside the sandbox | Execute user-task code and document-processing scripts in isolation. | Python/JS/TS tasks, document workflows, and sandbox-based document imports. |
| External model and MCP services | Perform search, speech, image, music, video, animation, or other specialized operations. | The capabilities you choose to enable. Their dependencies are managed separately. |

**Python is not only a testing dependency.** Host Python runs the supplied operational scripts; sandbox Python executes isolated tasks. A directly launched, prebuilt Rust executable does not need a host Python interpreter for basic chat and remote MCP calls. Installing packages in a host Conda environment does not make them available inside the task sandbox.

## Build and deploy

The supplied build scripts target Linux / WSL2. Install the toolchains above and native compiler/linker dependencies such as `build-essential`, `pkg-config`, and `libssl-dev` on Debian / Ubuntu.

```sh
git clone git@github.com:jason571/Exoharness.git
cd Exoharness
cargo fetch --locked
npm --prefix web-console ci
bash scripts/package-release.sh
```

The build produces `release/` with the executable, web resources, startup scripts, and a checksum manifest. Frontend resources are also embedded in the executable, so frontend changes require rebuilding the release. Credentials, databases, user files, and installed capability packages remain outside the release.

For sandbox tasks, build the separate execution image:

```sh
docker build -t exoharness-sandbox:0.1 runtime/sandbox
export EXOHARNESS_SANDBOX_IMAGE=exoharness-sandbox:0.1
```

The complete database and startup procedures are in the [Chinese deployment guide](README.zh-CN.md#准备数据库). The platform supports a native main service or a container-hosted main service. Currently, full OCI code execution uses the native main-service mode; an external sandbox execution proxy for the container-hosted mode is not yet connected.

Default local endpoints:

- Management: `http://127.0.0.1:9797`
- User portal: `http://127.0.0.1:9798`
- File service: `http://127.0.0.1:9790`

Public deployments should use an HTTPS reverse proxy and follow the current [Host/Origin, credential, and database security boundaries](docs/SECURITY.md).

## Try it and join the community

Visit the [live demo](https://agent.clhh.tech/) and register or sign in to use the configured assistants. Avoid uploading sensitive information to the public demo.

Scan the QR code to join the ExoHarness WeChat technical discussion group. Its expiry date is shown in the image; the image will be updated when needed.

<img src="docs/assets/wechat-group.jpg" alt="ExoHarness WeChat community QR code" width="300" />

## Documentation and current scope

Full technical documentation is currently in Chinese; English documentation will be added later.

- [Architecture](docs/ARCHITECTURE.md), [API](docs/INTERFACES.md), [Database](docs/DATABASE.md)
- [Context and Token management](docs/CONTEXT_AND_TOKENS.md), [Capabilities](docs/CAPABILITIES.md), [Coding](docs/CODING_WORKFLOW.md)
- [Deployment](docs/DEPLOYMENT.md), [Security](docs/SECURITY.md), [Testing](docs/TESTING.md)
- [Implementation status and limitations](docs/STATUS.md), [Documentation index](docs/README.md), [Contributing](CONTRIBUTING.md)

Read the implementation status before planning a deployment. In particular, safe HTTP Run Resume, the user self-installation execution chain, managed stdio MCP execution, and dynamic code-plugin loading are not yet connected.

This repository contains the core framework, frontend, tests, build/deployment scripts, and documentation. Root-level `skills/` and `mcp/` capability packages are maintained separately. The framework integration modules `modules/skills` and `modules/mcp` are included.

## License

Core code is licensed under the [MIT License](LICENSE). External Skills, MCP services, runtime images, model weights, and third-party dependencies retain their own licenses.
