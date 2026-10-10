# Skills

English | [简体中文](README.zh-CN.md)

`exoharness-skills` manages Skill package import, revisions, instructions, and resource reads. Package contents are stored by digest, registration records reference a revision, and execution reads a snapshot pinned to that revision.

## Main interfaces

- [service.rs](src/service.rs): `Skills::preview` checks incoming content; `import` stores the package and registers a revision; `instructions` and `instruction_view` read guidance; `files` lists resources; and `remove` handles deletion.
- [package.rs](src/package.rs), [archive.rs](src/archive.rs): decode input and validate package paths and archive structure.
- [metadata.rs](src/metadata.rs), [normalization.rs](src/normalization.rs): parse `SKILL.md` metadata and normalize the import layout.
- [snapshot.rs](src/snapshot.rs): `PackageSnapshot` pins package identity and resources for execution.
- [storage.rs](src/storage.rs): stores package files and digests.

Import reads, validates, and stores content. Script execution separately requires environment checks and execution authorization. Resource reads are constrained to package paths and the current access scope. Deletion considers both registration records and package references. Task instructions, reference material, and scripts are part of the Skill package.

## Tests

[tests/packages.rs](tests/packages.rs) checks package format and paths; [tests/service.rs](tests/service.rs) verifies registration, resources, and revisions. Service tests require a dedicated PostgreSQL database:

```sh
cargo test --locked -p exoharness-skills
```

See [Testing and acceptance](../../docs/TESTING.md) for environment setup and [Capabilities](../../docs/CAPABILITIES.md) for integration.
