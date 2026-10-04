## Context

Issue #14 asks for agent-friendly docs and endpoints, plus a way to keep both current as bevaru grows. Today:
- **Capabilities are spread across Rust types:** `LossKind`, `ModelKind`, `DatasetChoice`/`View`, `SweepParam`, and the experience registry. The messages that drive the app are `LoadExperiment`, `PlaybackCommand`, `SweepCommand`, `EnterExperience`, and `LeaveExperience`.
- **No machine-readable description exists.** There's no `AGENTS.md` or `llms.txt`, and nothing serializes (no serde).
- **No way to try things without code:** using any capability means writing a Bevy app.

**External standards** (checked 2026-10-03):
- **MCP:** the official Rust SDK is `rmcp` 3.5.
- **A2A:** protocol 1.0 was finalized on 2026-03-12 (current release v1.0.1). Agent Cards are published at `/.well-known/agent-card.json`; their normative definition is `specification/a2a.proto` (see decision 7).
- **AGENTS.md:** the vendor-neutral convention for repo instructions to coding agents, read by Claude Code, Codex, Cursor, Copilot and others.
- **llms.txt:** the convention for an index of docs meant for LLMs.
- **Bevy Remote Protocol:** Bevy 0.19 ships BRP (`bevy_remote`), a JSON-RPC 2.0 server that runs inside the app and supports custom methods.

## Goals / Non-Goals

**Goals:**
- An agent can learn everything bevaru offers from one JSON document, and from docs that link to it.
- An agent can try any headless capability through MCP, with no Rust toolchain and no GPU.
- An agent can drive a running bevaru window.
- When code and docs disagree, a build or test fails. Nobody has to remember to update the docs.

**Non-Goals:**
- **A2A, in this change** (neither a server nor an Agent Card). bevaru is a tool, not an autonomous agent; MCP is the right protocol for tools. Decision 7 explains why a card can't be published without an A2A endpoint.
- **Network-exposed services.** Everything is stdio or localhost and opt-in.
- **Generating Rust API docs.** `cargo doc` already does that; this change makes sure the agent docs point at it.
- **Exposing the egui control panel's every widget remotely.** Remote methods map to the same messages a program would send.

## Decisions

### 1. One manifest, built from the code, in the `bevaru` crate
`bevaru::agent::manifest(&ExperienceRegistry) -> Manifest`: plain `Serialize` structs with owned strings.
- **Exhaustive matches:** loss, model, and sweep-parameter descriptions come from `match` expressions with no wildcard arm. A new variant fails to compile until it is described, which is the compile-time half of the freshness guarantee.
- **Stable ids:** each enum gains `id()` / `from_id()`, with kebab-case ids: `hinge`, `squared-hinge`, `linear-regression`, `huber-delta`, and so on. They're separate from the display names, so renaming a label never breaks an agent. A round-trip test covers every variant.
- **Experiences** come from the registry the caller passes in, so third-party experiences appear automatically.
- **Why the `bevaru` crate, not `bevaru-core`:** the manifest needs the experience registry and the message types, which live in `bevaru`. Core only gains the `id`/`from_id` methods, with no new dependency.
- **Alternative:** hand-written YAML or JSON. That's exactly the drift the issue wants to avoid.

### 2. An agent-facing wire format, separate from internal types
`bevaru::agent::api` defines the request types agents send. They derive `Deserialize` and `JsonSchema` (schemars 1.x):
- `ExperimentRequest`, `TrainerRequest`, `SweepRequest`, `LossEvalRequest`, `DatasetRequest`;
- each has a `TryFrom` into the library type, which validates through the existing `TrainerConfig::validate` and `LossParams`.

Both the MCP tools and the remote methods use these types, so there's one schema and one validator.
- **Why not derive serde on `TrainerConfig` and friends:** internal types change shape for implementation reasons, such as `LearningRate`'s variants or private fields. A wire format must stay stable, and must reject bad input with field-level messages. The `TryFrom` boundary gives both.
- **Cost:** a small amount of mapping code, which tests keep honest. Every built-in experience's spec has to round-trip through `ExperimentRequest`.

### 3. Docs layout, and generation inside the library
- **Hand-written:**
  - `AGENTS.md`, for agents working in the repository;
  - `llms.txt`, the index for agents using bevaru;
  - `docs/agents/guide.md`;
  - `docs/agents/recipes/*.md`;
  - `docs/agents/mcp.md`.
- **Generated:** `docs/agents/capabilities.json` and `docs/agents/capabilities.md`, each marked "generated — do not edit", plus the code blocks that docs include from compiled examples.
- **One code path:** the generators are functions in `bevaru::agent::docs`, returning `(path, contents)` pairs. `cargo run -p bevaru-mcp -- gen-docs` writes them, and the freshness test in `tests/agent_docs.rs` compares them with the committed files.
- **Why the test is in the `bevaru` crate:** CI already runs `cargo test -p bevaru`, so the check can't be skipped. When it fails, it prints the stale paths and the regeneration command, plus a short diff of the first difference.
- **Link checking:** the same test checks that every relative link in `llms.txt`, `AGENTS.md`, and `docs/agents/**/*.md` points at an existing file. Moving docs can then never break the index.

### 4. The tool catalog is data; the MCP crate implements it
`bevaru::agent::tools()` lists every tool: name, group, description, and input schema. The manifest includes it, so the docs describe tools without depending on the MCP crate. `crates/bevaru-mcp` implements the handlers, and a test checks that the server's registered tool names equal the catalog exactly, in both directions (spec: "Tools and manifest agree").

### 5. `bevaru-mcp`: rmcp over stdio, compute off the async runtime
- **Crate:** a new workspace member that depends on `bevaru`, `rmcp` (server and stdio transport), `tokio`, and `serde_json`. It forwards an `mnist` feature.
- **Execution:** training and sweeps run in `spawn_blocking`. Charts reuse `bevaru::charts::LineChart::render`, encoded to PNG and returned as MCP image content.
- **Limits:** keep responses bounded and the server responsive:
  - at most 100 000 steps per call;
  - at most 50 sweep values;
  - trajectories downsampled to 500 points;
  - dataset coordinates capped at 5 000 points.

  Each limit is documented in its tool description.
- **Resources:** `bevaru://capabilities.json`, `bevaru://capabilities.md`, and `bevaru://docs/<path>`. They're embedded with `include_str!`, so they always match the binary.
- **Headless registry:** the server builds a minimal `App` with `ExperiencesPlugin` to read the registry. That's the same trick `src/main.rs` uses for `--list`, and it needs no window.
- **API verification:** `rmcp` 3.x is newer than the versions I know, so the exact macro and handler API is checked against its source and examples when implementing, as was done for Bevy 0.19.

### 6. Live-app hooks through Bevy Remote Protocol
Behind `remote` (which enables Bevy's `bevy_remote`), `RemoteHooksPlugin`:
- adds BRP's `RemotePlugin` and its HTTP transport, bound to 127.0.0.1;
- registers `bevaru.experiences.list`, `bevaru.experiences.enter`, `bevaru.experiences.leave`, `bevaru.playback`, `bevaru.sweep.start`, `bevaru.sweep.stop`, and `bevaru.state`.

Each method is a system that sends the existing messages (`EnterExperience`, `PlaybackCommand`, …), so remote control goes through exactly the paths the UI uses: same clean-up, same validation. The `bevaru` binary enables the plugin only with `--remote`.

`bevaru-mcp --app <url>` adds `app_*` tools that forward to these methods over HTTP JSON-RPC.
- **Why BRP rather than a custom socket:** it's built into Bevy, it already speaks JSON-RPC, and it gives agents generic entity and component inspection for free alongside the `bevaru.*` methods.

### 7. No A2A Agent Card until bevaru serves A2A
**Decided 2026-10-03, with the maintainer:** publish no Agent Card in this change.

- **The evidence:** checked against A2A v1.0.1 itself, not summaries:
  - `specification/a2a.proto`, the normative definition, marks `AgentCard.supported_interfaces` as `REQUIRED`;
  - `docs/specification.md` §5 says "Arrays marked as required **MUST** contain at least one element";
  - each entry must be a URL serving the A2A protocol over a binding (`JSONRPC`, `GRPC`, `HTTP+JSON`, or an extension of those).
- **Why that rules out a card here:**
  - bevaru serves no A2A endpoint, so a card with an empty interface list would be invalid, and conformant clients should reject it;
  - pointing an interface at the MCP server would be false, because MCP is a different protocol, not an A2A binding.
- **Discovery without a card:** agents find bevaru through `llms.txt`, `capabilities.json`, and the MCP server, which already cover everything a card would have listed.
- **Options considered:**
  - an empty interface list: invalid, see above;
  - a skill list that isn't called an A2A card: redundant with `capabilities.json`;
  - **building an A2A endpoint**, a localhost JSON-RPC service running the same `bevaru::agent::run` tools: deferred. It's a new network service, and bevaru's request/response tools map awkwardly onto A2A's message-and-task model.
- **To add one later:**
  - build that endpoint, reusing the tool catalog and `run` (the same split that made the MCP server thin);
  - generate the card from the manifest, with one skill per `tools::GROUPS` entry;
  - add it to the freshness test and the MCP resources.

### 8. Rollout
Each phase is useful on its own and can merge separately:
1. **The manifest, ids, wire types, and generated docs with the freshness test.** This is the foundation everything else reads, and the part that keeps docs current.
2. **The `bevaru-mcp` headless server.**
3. **The remote hooks and the MCP bridge.**
4. ~~The Agent Card~~ (dropped; see decision 7).

## Risks / Trade-offs

- **[`rmcp` 3.x API differs from what I expect]** → verify from source before writing handlers (task 3.1). The tool catalog (decision 4) keeps tool definitions independent of the SDK's macros, so an SDK change touches only the handler layer.
- **[Generated files cause merge conflicts]** → they're deterministic (sorted keys, stable ordering) and regenerated by one command. A conflict is resolved by regenerating, which `AGENTS.md` states.
- **[Heavy tool calls block the server]** → `spawn_blocking`, hard limits per call, and documented limits.
- **[Remote control is a local attack surface]** → off unless both the feature and `--remote` are given; localhost only; it can only do what the UI can. Documented in `docs/agents/mcp.md`.

## Open Questions

- ~~A2A `supportedInterfaces`~~: resolved. There's no card until bevaru serves A2A (decision 7).
- **Should `bevaru-mcp` be published to crates.io** or installed from the repository with `cargo install --git`? It affects the install instructions in `docs/agents/mcp.md`.
- ~~Remote port~~: resolved. Keep BRP's standard 15702, which existing BRP tools expect, and change it with `--remote-port`.
