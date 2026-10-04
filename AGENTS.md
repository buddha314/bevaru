# AGENTS.md

Instructions for AI coding agents working in this repository. If you are building an application *with* bevaru, start at [llms.txt](llms.txt) instead.

## What this is

bevaru is a Bevy plugin for interactive, animated machine-learning visualizations (loss functions, linear models, SVM margins, sweeps), with charts drawn by ruviz. `cargo run` opens a lobby of experiences.

| Path | Contents |
| ---- | -------- |
| `crates/bevaru-core` | Everything without Bevy: losses and gradients, linear models trained one step at a time, datasets, PCA, the optional MNIST loader. |
| `src/` | The Bevy plugin. `experiment.rs` (loading), `playback.rs` (training playback, sweeps), `scene.rs`, `charts.rs`, `controls.rs` (egui), `experiences/` (registry and built-ins), `lobby.rs`, `agent/` (manifest, wire format, tool catalog, doc generators). |
| `src/main.rs` | The `bevaru` binary: the lobby, `-- <id>`, `--list`, `--thumbnail`. |
| `crates/bevaru-mcp` | The MCP server for agents (stdio; tools dispatch to `src/agent/run.rs`, and `--app` forwards `app_*` tools to a running window) and `gen-docs`. |
| `src/remote.rs` | `bevaru.*` Bevy Remote Protocol methods (feature `remote`); their list lives in `agent::REMOTE_METHODS`. |
| `examples/` | One-line wrappers that open experiences, plus `headless_training` and `custom_experience`, whose code the recipes include. |
| `docs/agents/` | Docs for agents using bevaru. `capabilities.*` are generated. |
| `openspec/` | Specs (`specs/`) and change proposals (`changes/`). |

## Commands

```sh
cargo build --workspace                       # build everything
cargo test --workspace                        # all tests (headless; no window or GPU needed)
cargo fmt --all --check                       # formatting
cargo clippy --workspace --all-targets --features mnist -- -D warnings   # lint, as CI does
cargo run -p bevaru-mcp -- gen-docs           # regenerate docs/agents/ reference files
cargo run --release                           # the app (needs a display and a GPU)
cargo run --features remote -- --remote       # the app, controllable over BRP on 127.0.0.1:15702
cargo test -p bevaru --features remote        # also runs the remote-method tests
```

On Linux, building needs `pkg-config` and the udev, wayland and xkbcommon development packages (see the README). The first build compiles Bevy and takes several minutes.

## Rules that tests enforce

- **Generated docs are never edited by hand.** `docs/agents/capabilities.json`, `docs/agents/capabilities.md`, and the code blocks marked `<!-- include: … -->` in `docs/agents/recipes/` come from the code. After changing a loss, model, dataset, sweep parameter, experience, message, tool, request type, or an included example, run `cargo run -p bevaru-mcp -- gen-docs`. `tests/agent_docs.rs` fails otherwise. If generated files conflict in a merge, regenerate them rather than resolving by hand.
- **Describe new capabilities.** Adding a `LossKind`, `ModelKind`, or `SweepParam` variant fails to compile in `src/agent/mod.rs` until it is described there, and needs an id in its `id()` match.
- **Tools and the catalog agree.** A tool exists in `src/agent/tools.rs`, runs in `src/agent/run.rs`, and is dispatched in `crates/bevaru-mcp/src/main.rs`; tests fail if any of the three is missing. Every file in `docs/agents/` must also be served as an MCP resource (`RESOURCES` in the server).
- **Links must resolve.** Every relative link in `llms.txt`, `AGENTS.md`, and `docs/agents/` is checked.
- **Leaving an experience leaves nothing behind.** Spawn a custom experience's entities with `ExperienceEntity` and remove its resources on `ExperienceStopped`. Per-experiment state is cleared on `UnloadExperiment` in `PreUpdate`. The ten-round-trip leak test in `src/lobby_tests.rs` catches anything missed.
- **`cargo run` opens the lobby.** `tests/default_run.rs` keeps it that way.

## Conventions

- **Bevy 0.19 and bevy_egui 0.42 are newer than many models' training data.** Buffered events are `Message`s (`MessageReader` / `MessageWriter`); `Event` is for observers. egui panels are `egui::Panel` shown inside a `Ui`. When unsure of an API, read the crate source under `~/.cargo/registry/src/` rather than guessing.
- **Test headlessly.** `src/lobby_tests.rs` shows how to run the real scene and chart plugins without a GPU (`MinimalPlugins`, `AssetPlugin`, `GizmoPlugin`, and the mesh, material, and image assets).
- **Math belongs in `bevaru-core`**, which must not depend on Bevy (CI checks this). Numeric code there is optimized even in dev builds.
- **Ids are kebab-case and stable:** loss, model, sweep-parameter, and experience ids are part of the agent API. Don't rename them.
- **Specs first for larger changes:** propose with OpenSpec (`openspec/changes/<name>/`), implement against its `tasks.md`, then archive it so `openspec/specs/` stays current.
