# Assistant configuration

English | [简体中文](ASSISTANT_CONFIGURATION_DESIGN.zh-CN.md)

An assistant defines its identity, avatar, system prompt, default model and capability scope. Administrators assign assistants to users; the regular chat page does not let a user switch to another assistant.

## Configuration

- Identity, name and system prompt describe the role and interaction style. They do not claim that every execution capability is available.
- An avatar is stored as a content-addressed asset and displayed through a stable reference. Uploading an avatar updates the selected assistant; it does not create another assistant.
- Skill, MCP, Tool and model-backed capabilities are authorized separately. Selecting a Skill does not grant the MCP services it may depend on.
- The default LLM and speech model are configured separately. Type defaults and execution authorization are checked independently.
- Only enabled assistants are selectable for user assignment. Deletion is constrained by the current default and existing references.

## Updates and execution

Updating an assistant's current configuration creates an internal immutable generation/revision without creating a new assistant identity. Existing execution snapshots continue to reference their original revision; the management UI shows the current assistant rather than presenting old revisions as duplicate assistants.

A new session fixes the public snapshot of its assigned assistant. Each Run further fixes the assistant revision and execution-capability snapshot. A snapshot does not bypass current authorization revocation, capability enablement or runtime-readiness checks.

Tool discovery reads live schemas from the authorized capability catalog. The role prompt defines identity and communication style; the capability catalog defines what can actually be called. Missing authorization, missing registration, missing runtime dependencies and service execution errors use separate error categories.

See the [management API](INTERFACES.md). Implementation is in `configuration/src/profiles/`, `api/src/configuration/` and the `identity` user-assignment service.
