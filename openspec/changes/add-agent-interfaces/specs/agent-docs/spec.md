## ADDED Requirements

### Requirement: Documentation layout for agents
The repository SHALL provide:
- `AGENTS.md` at the root, for coding agents working in the repository: how to build, test, and lint; the crate layout; the conventions; and where to change things;
- `llms.txt` at the root, following the llms.txt convention: a short description of bevaru and links to the agent docs;
- `docs/agents/`, holding:
  - a usage guide for building applications with bevaru (adding the plugin, choosing an experience or experiment, driving it with messages, running headless);
  - recipes for common tasks (add an experience, embed the lobby, train headlessly, use the MCP server, drive a running app);
  - the generated reference files.

#### Scenario: An agent starting from llms.txt
- **WHEN** an agent reads `llms.txt`
- **THEN** it finds links to the usage guide, the recipes, the generated capability reference, and the MCP server instructions, all of which resolve within the repository

#### Scenario: AGENTS.md commands work
- **WHEN** the build, test, and lint commands listed in `AGENTS.md` are run in a clean checkout
- **THEN** they succeed

### Requirement: Generated reference
`docs/agents/capabilities.json` (the serialized manifest) and `docs/agents/capabilities.md` (a readable rendering of it) SHALL be generated from the manifest by one documented command. They SHALL NOT be edited by hand, and each SHALL say so in a header.

#### Scenario: Regeneration
- **WHEN** a developer runs the documented generation command
- **THEN** both reference files are rewritten from the current code, and running it again produces no changes

### Requirement: Docs stay current
A test SHALL fail when any generated artifact (`capabilities.json`, `capabilities.md`, or a code block included into a doc from a compiled example) differs from what the current code generates. Its failure message SHALL name the stale files and the command that regenerates them. The test SHALL run in CI.

#### Scenario: Capability added without regenerating
- **WHEN** a developer registers a new experience, or adds a loss, and doesn't regenerate the docs
- **THEN** the test suite fails, naming `docs/agents/capabilities.json` and the generation command

#### Scenario: Hand edit to a generated file
- **WHEN** someone edits `docs/agents/capabilities.md` by hand
- **THEN** the test suite fails until the file is regenerated
