## Why

bevaru is meant to be used by many client applications, most of them likely built by AI agents ([#14](https://github.com/buddha314/bevaru/issues/14)). Today an agent has to read the Rust source to learn what bevaru can do. Nothing describes the library in a form an agent can query, and there is no way to try a capability (evaluate a loss, train a model, render a chart) without writing and compiling Bevy code. As capabilities grow, hand-written docs would also drift. The issue asks for three things: a doc structure for agents, endpoints, and a mechanism that keeps both current.

## What Changes

- **A capability manifest:** a single machine-readable description of what bevaru offers, built from the code itself, so it can't fall behind it. It covers:
  - losses with their hyperparameters and ranges;
  - models;
  - datasets and views;
  - sweep parameters;
  - registered experiences;
  - the plugin's public messages and commands.

  It's built with exhaustive `match`es, so adding a loss, model, or sweep parameter without describing it fails to compile. It serializes to JSON with JSON Schemas for tool inputs.
- **Docs for agents**, in a fixed layout:
  - `AGENTS.md` at the root: building, testing, and conventions, for coding agents working on this repo;
  - `llms.txt` at the root: an index of the docs for agents using bevaru;
  - `docs/agents/`: a usage guide, recipes, and **generated** reference files (`capabilities.md`, `capabilities.json`) produced from the manifest.
- **An MCP server**, `bevaru-mcp` (stdio). It exposes the manifest and docs as resources, plus headless tools that need no GPU or window:
  - describe capabilities and list experiences;
  - evaluate losses;
  - build a dataset view;
  - train a model and return its trajectory;
  - run a hyperparameter sweep;
  - render a loss or training chart as PNG.
- **Live-app hooks:** behind a `remote` feature, the bevaru app serves Bevy Remote Protocol methods (`bevaru.*`) to list, enter, and leave experiences, control playback, start sweeps, and read the current state. `bevaru-mcp --app <url>` exposes these as tools, so an agent can drive a running window.
- **No A2A Agent Card yet.** A valid card must list at least one A2A endpoint, and bevaru has none. Agents discover bevaru through `llms.txt`, the manifest, and the MCP server instead. See the design for the evidence and the path to adding one.
- **Freshness:**
  - every generated artifact is reproduced by `cargo run -p bevaru-mcp -- gen-docs`;
  - a test fails, showing the regeneration command, whenever a committed artifact differs from what the code would generate;
  - consistency tests tie every MCP tool to a manifest entry and every manifest capability to a doc section.

## Capabilities

### New Capabilities
- `capability-manifest`: The code-derived, serializable description of bevaru's capabilities, with compile-time and test-time completeness guarantees.
- `agent-docs`: The documentation layout for agents (AGENTS.md, llms.txt, `docs/agents/`), the generated reference, and the freshness checks.
- `mcp-server`: The `bevaru-mcp` stdio server: resources, headless tools, and their input schemas.
- `app-remote`: Bevy Remote Protocol methods for driving a running bevaru app, and their MCP bridge.

### Modified Capabilities
<!-- none: existing behaviour is unchanged; the remote feature is opt-in -->

## Impact

- **Code:**
  - new `src/agent/` (the manifest);
  - new crate `crates/bevaru-mcp` (server binary and doc generator);
  - `src/remote.rs` behind the `remote` feature;
  - `src/main.rs` gains `--remote`.
- **Dependencies:**
  - `serde` and `serde_json` (the manifest; `bevaru-core` types gain `Serialize` behind a `serde` feature);
  - `schemars` (tool input schemas);
  - `rmcp` and `tokio` in `bevaru-mcp` only;
  - Bevy's `bevy_remote` feature behind `remote`.
- **Docs:** new `AGENTS.md`, `llms.txt`, and `docs/agents/`. The README gains an "For agents" section.
- **CI:** the existing test job runs the freshness test; a build step checks `bevaru-mcp` and `--features remote`.
- **Security:**
  - the MCP server is stdio-only and local;
  - the remote feature is off by default and binds to localhost;
  - nothing is exposed on a network interface unless the user opts in.
