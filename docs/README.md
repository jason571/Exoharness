# ExoHarness documentation

English | [简体中文](README.zh-CN.md)

This documentation describes the ExoHarness architecture, APIs, persistence, execution policies and operations. [Current feature status](STATUS.md) distinguishes implemented paths from limitations.

| Document | Scope |
| --- | --- |
| [Commercial licensing](../COMMERCIAL-LICENSE.md) | AGPL terms and the separate commercial-license option |
| [Architecture](ARCHITECTURE.md) | Module responsibilities, extension lifecycle, execution flow and invariants |
| [API reference](INTERFACES.md) | Management, user and file routes, authentication, requests and events |
| [Database](DATABASE.md) | Table groups, transactions, configuration files, migrations and backup boundaries |
| [Context and Tokens](CONTEXT_AND_TOKENS.md) | Execution journal, model projection, on-demand capabilities, checkpoints and accounting |
| [Capabilities](CAPABILITIES.md) | Skill/MCP import, preparation, authorization, schemas, progress and concurrency |
| [Model configuration](MODEL_CONFIGURATION_DESIGN.md) | LLM/TTS/ASR protocols and assistant model selection |
| [Assistant configuration](ASSISTANT_CONFIGURATION_DESIGN.md) | Identity, avatars, capability grants and user assignment |
| [Coding workflow](CODING_WORKFLOW.md) | Multi-file coding, focused repair, checks and delivery |
| [Knowledge import](KNOWLEDGE_DOCUMENT_IMPORT.md) | Categories, DOCX/text PDF, tables and retrieval boundaries |
| [Artifact delivery](MEDIA_DELIVERY_DESIGN.md) | Files, signatures, media playback and web previews |
| [Web console](WEB_CONSOLE.md) | Page assets, languages, data boundaries and browser coverage |
| [Deployment](DEPLOYMENT.md) | Native builds, release package, standalone/container operation |
| [Security](SECURITY.md) | Tenant isolation, credentials, sandboxing, network and public access |
| [Testing](TESTING.md) | Unit, database, protocol, OCI, browser and real-task acceptance |
| [Migration](MIGRATION.md) | Reusing historical configuration/data and Run-format boundaries |
| [Feature status](STATUS.md) | Connected functionality and current limitations |
| [Open-source checklist](OPEN_SOURCE_CHECKLIST.md) | Private data, licenses, publication and review steps |
| [Release notes template](RELEASE.txt) | Standalone release startup instructions |
| [Delivery testing](TESTING.md#critical-regressions) | Artifact delivery and failure recovery coverage |
| [File/concurrency testing](TESTING.md#coverage-levels) | Signed-link policy and admission test layers |
| [Tool discovery testing](TESTING.md#execution-and-file-delivery-regressions) | Discovery and authorization regression gates |

`legacy-api-inventory.json` records routes from a historical version for migration review. Current route behavior is documented in the [API reference](INTERFACES.md), with implementation under `modules/api/src/`.
