## Why

`cargo run` currently drops straight into one hard-wired experience (the Iris C sweep), and the other experiences only exist as separate `--example` binaries that a user has to know about. Issue [#12](https://github.com/buddha314/bevaru/issues/12) asks for a lobby: a start screen where users pick which simulation or experience they want. It makes the project discoverable from a single `cargo run`, and gives every future visualization a place to appear.

## What Changes

- Add a **lobby screen** shown when the `bevaru` binary starts: one card per experience with its title, a one-line description of what it teaches, and any requirements (e.g. MNIST needs the `mnist` feature and a one-time ~11 MB download).
- Add an **experience catalogue** in the library: each experience (Iris SVM C-sweep, MSE vs MAE vs Huber, loss curves, MNIST 3-vs-8) is defined once — its experiment spec, title, description, and what to do on start (play training, run a sweep). The lobby and the existing `--example` binaries both use it, so they cannot drift apart.
- Picking a card loads that experience; a **"Back to lobby"** control (and `Esc`) returns to the lobby, tearing down the scene, charts, running training, sweeps, and any in-flight load.
- Experiences unavailable in the current build (MNIST without the feature) are shown disabled with the reason, not hidden.
- `cargo run -- <experience-id>` skips the lobby and opens that experience directly; `cargo run -- --list` prints the ids.
- `BevaruPlugin` gains the ability to start **idle**, with no experiment loaded, which the lobby requires. Today it always loads a default experiment at startup.
- `examples/*.rs` become thin wrappers over catalogue entries. Their behaviour is unchanged.

## Capabilities

### New Capabilities
- `experience-lobby`: The experience catalogue, the lobby screen, selecting and leaving experiences, availability gating, and command-line selection.

### Modified Capabilities
- `visualization-scene`: The "Bevy plugin entry point" requirement gains an idle start mode (no experiment loaded until one is requested), and the scene must be fully torn down when an experience is left.

## Impact

- **Code**: new `src/experiences.rs` (catalogue) and `src/lobby.rs` (state, UI, teardown); `src/main.rs` becomes the lobby app; `src/experiment.rs` startup behaviour; `src/scene.rs`, `src/charts.rs`, `src/controls.rs`, `src/playback.rs` gain run conditions/teardown for leaving an experience; `examples/*.rs` slimmed to catalogue lookups.
- **APIs**: additive. `ExperimentSpec`, `StartupExperiment`, and the examples keep working; new public `Experience`, `EXPERIENCES`, `LobbyPlugin`, `AppScreen` state.
- **Dependencies**: none new (lobby UI uses the existing `bevy_egui`).
- **Docs**: README quick start changes from "the default run is Iris" to "pick from the lobby".
