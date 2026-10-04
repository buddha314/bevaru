## 1. Capability manifest

- [x] 1.1 Add stable kebab-case `id()` / `from_id()` to `LossKind`, `ModelKind` (bevaru-core) and `SweepParam` (bevaru), via exhaustive matches; round-trip tests over every variant
- [x] 1.2 Add `serde`, `serde_json`, `schemars` to `bevaru`; create `src/agent/mod.rs` with the `Manifest` types and `schema_version`
- [x] 1.3 Build the manifest from code: losses (task, trainability, hyperparameters and ranges), models (task, losses), datasets and views, sweep parameters (where they apply), experiences from a registry, and control messages
- [x] 1.4 Add `src/agent/api.rs` wire types (`ExperimentRequest`, `TrainerRequest`, `SweepRequest`, `LossEvalRequest`, `DatasetRequest`) with `JsonSchema` and validating `TryFrom` into library types
- [x] 1.5 Add the tool catalog `agent::tools()` (name, group, description, input schema) and include it in the manifest
- [x] 1.6 Tests: one entry per loss/model/sweep parameter; third-party experience appears; JSON round trip; every built-in experience's spec round-trips through `ExperimentRequest`; invalid requests name the field

## 2. Docs for agents and freshness

- [x] 2.1 Add generators in `src/agent/docs.rs`: `capabilities.json` (pretty, stable order), `capabilities.md` (readable tables), each with a "generated — do not edit" header
- [x] 2.2 Write `AGENTS.md` (build, test, lint, layout, conventions, regenerating docs) and `llms.txt` (index)
- [x] 2.3 Write `docs/agents/guide.md` and recipes: add an experience (which covers embedding the lobby) and train headlessly *(as built: recipe and guide code is included from compiled examples, `minimal_app`, `headless_training`, and `custom_experience`, and kept current by the freshness test; the MCP and remote-control recipes move to 3.6 and 4.4, alongside the features they describe)*
- [x] 2.4 Add `tests/agent_docs.rs`: generated files match the code (failure names stale files and the command); every relative link in `llms.txt`, `AGENTS.md`, `docs/agents/**` resolves
- [x] 2.5 Generate and commit the reference files; README gains a "For agents" section

## 3. MCP server

- [x] 3.1 Verify the `rmcp` 3.x server API (tool registration, stdio transport, image content, resources) from its source and examples *(dispatched to a subagent; key differences from older rmcp: `ContentBlock`, `...Params` types, `CallToolResponse`/`ReadResourceResponse` enums, builders for `#[non_exhaustive]` structs; tools are served from the catalog with manual `list_tools`/`call_tool`, not macros)*
- [x] 3.2 Create `crates/bevaru-mcp` (workspace member; `mnist` feature) with a stdio server and the `gen-docs` subcommand *(stdio server done)*
- [x] 3.3 Implement tools: describe, list experiences, evaluate losses, build a dataset view, train, sweep, render a chart (PNG); compute in `spawn_blocking`; enforce and document limits *(execution lives in the library, `bevaru::agent::run`, for reuse by the remote hooks; sweeps train values in parallel; results are object-shaped structured content as MCP requires; schemas are post-processed to drop `null` types, which cleared all 41 portability warnings from the MCP Inspector)*
- [x] 3.4 Serve embedded resources: capabilities and docs *(capabilities, guide, MCP doc, and recipes; a test requires every `docs/agents/` file to be served; the Agent Card is added in 5.2)*
- [x] 3.5 Tests: the tool names equal the catalog; hinge evaluation values; SVM training result; Huber δ = 0 error; chart returns a PNG; an end-to-end stdio session (initialize, list tools, call one) *(end-to-end over stdio with the real binary, also with no display; independently checked with the official MCP Inspector CLI, including an MNIST training call)*
- [x] 3.6 Write `docs/agents/mcp.md` and a recipe for using the MCP server (install, client configuration snippets, tool reference link), linked from `llms.txt`; add CI steps for `cargo test -p bevaru-mcp`

## 4. Live-app remote hooks

- [x] 4.1 Verify the `bevy_remote` 0.19 API (custom methods, HTTP transport, binding) *(handlers are systems taking `In<Option<Value>>` plus any system parameters; HTTP binds 127.0.0.1:15702 by default and is on in Bevy's `bevy_remote` for desktop targets; the `BrpSender` mailbox lets tests dispatch requests without a socket)*
- [x] 4.2 Add the `remote` feature and `RemoteHooksPlugin` with `bevaru.*` methods mapping to existing messages; localhost only; `--remote` flag on the binary *(default port is BRP's standard 15702, changeable with `--remote-port`; the method list lives in `agent::REMOTE_METHODS` outside the feature gate, so the generated docs list it in every build)*
- [x] 4.3 Add `app_*` tools to `bevaru-mcp` behind `--app <url>`, with clear unreachable-app errors *(validates arguments with the shared wire types before forwarding; found and fixed `app_playback`'s non-object input schema, and the catalog test now requires every tool input to be an object)*
- [x] 4.4 Tests: methods drive a headless app (enter, state, leave, unknown id); without `--app` the app tools aren't listed; build with `--features remote` in CI; write the "drive a running app" recipe *(also an end-to-end test of MCP client → `bevaru-mcp --app` → HTTP → a live headless app; and checked against a real window: `curl` switched it from the lobby to the Iris sweep, with both sockets bound to 127.0.0.1)*

## 5. A2A Agent Card

- [x] 5.1 Resolve the `supportedInterfaces` open question against the v1.0 spec *(resolved: A2A v1.0.1 requires at least one A2A endpoint and bevaru serves none, so no card is published in this change; generating it and testing it are dropped. See design decision 7)*

## 6. Verification

- [x] 6.1 Connect a real MCP client to `bevaru-mcp` and exercise every tool; drive a running window through `--app` *(all 8 headless tools, 7 resources, and all 7 `app_*` tools against a real `--remote` window, through the official MCP Inspector CLI as an independent client. Two runs lost the window: one was the laptop suspending mid-test (kernel log at 16:55:37); the other was the capture plugin exiting 1.5 s after requesting a screenshot even when none had arrived. That plugin now exits only once the image is written, or after 30 s with an error)*
- [x] 6.2 Follow `llms.txt` → guide → recipe from a clean checkout as an agent would, and fix anything that doesn't work as written *(in a copy of the tracked tree: every `llms.txt` link resolves, the guide's example builds, the headless recipe reproduces its documented output, the add-an-experience recipe shows its two new cards, `cargo test --workspace` passes, and `bevaru-mcp` installs and serves)*
