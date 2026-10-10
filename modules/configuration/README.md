# Configuration

English | [简体中文](README.zh-CN.md)

`exoharness-configuration` manages models, providers, assistants, avatars and configuration revisions. Management APIs write configuration, the execution layer pins a selected revision, and the user portal reads a filtered public view.

## Main interfaces

- [models/](src/models/): `Models` reads the catalog, saves configuration, enables/disables entries and sets defaults. `bind_enabled` returns a `ModelBinding` with model, provider and credentials.
- [profiles/](src/profiles/): `Assistants` creates, updates, enables, defaults and scopes assistants and manages avatars; `AssignmentLease` provides the configuration view used by assignment.
- [files.rs](src/files.rs): manages configuration generations and private files.
- [validation.rs](src/validation.rs): validates configuration structure.
- [lib.rs](src/lib.rs): `check_directory` validates a configuration directory.

Writes validate the expected revision; configuration and credentials are retrieved together when binding. Management and public-assistant views are projected separately. Credentials used by model execution are available only to the request adapter. Identity and callers coordinate assistant assignment and deletion-reference checks.

## Tests

[tests/](tests/) covers directory loading, revision conflicts, enablement, defaults, model binding and assistant writes using isolated temporary directories.

```sh
cargo test --locked -p exoharness-configuration
```

See [Model configuration](../../docs/MODEL_CONFIGURATION_DESIGN.md) and [Assistant configuration](../../docs/ASSISTANT_CONFIGURATION_DESIGN.md).
