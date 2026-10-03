## Why

`cargo run` currently drops straight into one hard-wired experience (the Iris C sweep). The other experiences only exist as separate `--example` binaries that a user has to know about. Issue [#12](https://github.com/buddha314/bevaru/issues/12) asks for a lobby: a start screen where users pick the simulation or experience they want. More experiences are coming soon, so they need a single place to register that the lobby, the examples, and the command line all read from.

## What Changes

- Add an **experience registry**:
  - `App::register_experience(...)` adds an experience; built-in and future plugins register the same way.
  - Each entry has an id, title, description, category, thumbnail, and build requirements.
  - An entry is either an **experiment experience** (a dataset + model spec and an on-start action) or a **custom experience** (its own plugin systems and entities, for things that aren't experiments, like the sigmoid plot).
- Register the five current experiences:
  - Iris SVM C sweep;
  - MSE vs MAE vs Huber;
  - loss curves;
  - MNIST 3 vs 8;
  - the ruviz **sigmoid** plot, as the first custom experience.
- Add a **lobby screen** shown when the `bevaru` binary starts:
  - cards grouped by category, each with a **thumbnail**, title, description, and requirements;
  - experiences unavailable in this build (MNIST without the `mnist` feature) are shown disabled, with the reason.
- **Navigation:**
  - picking a card starts that experience;
  - **"Back to lobby"** and `Esc` return to the lobby from every experience, custom ones included;
  - leaving tears down everything the experience created.
- **Clean-up on leave:**
  - however an experience is left (Back, `Esc`, a failed load, switching, or quitting), the app returns to its lobby baseline: no leftover entities, assets, egui textures, background tasks, or experience resources;
  - late results from abandoned loads, sweeps, and chart renders are discarded;
  - quitting stops the running experience first, and stale partial MNIST downloads are removed;
  - a ten-round-trip leak test enforces all of this.
- **Command line:** `cargo run -- <experience-id>` skips the lobby; `cargo run -- --list` prints the ids.
- **Plugin:** `BevaruPlugin` gains an **idle start** (no experiment loaded), which the lobby requires; today it always loads a default experiment.
- **Examples:** `examples/*.rs` become thin wrappers that start a registered experience by id. Their behaviour is unchanged.
- **Thumbnails:** small PNGs embedded in the binary, with a documented command to regenerate them.

## Capabilities

### New Capabilities
- `experience-lobby`: The experience registry (experiment and custom kinds), the lobby screen with thumbnails and categories, starting and leaving experiences, clean-up on leave and on quit, availability gating, and command-line selection.

### Modified Capabilities
- `visualization-scene`: The "Bevy plugin entry point" requirement gains an idle start mode. A new requirement says unloading an experiment must remove everything it created.

## Impact

- **Code**:
  - **New files:** `src/experiences/` (registry, built-in entries, sigmoid experience) and `src/lobby.rs` (state, UI, navigation, teardown).
  - **Rewritten:** `src/main.rs` becomes the lobby app.
  - **Changed:** `src/experiment.rs` (startup mode, unload); `src/scene.rs`, `src/charts.rs`, `src/controls.rs`, `src/playback.rs` (teardown and screen gating).
  - **Slimmed:** `examples/*.rs`.
- **APIs**: additive. Existing apps and `StartupExperiment` keep working. New public items: `Experience`, `ExperienceKind`, `ExperienceRegistry`, `RegisterExperience`, `LobbyPlugin`, `AppScreen`, `ActiveExperience`, `ExperienceEntity`.
- **Assets**: `assets/thumbnails/*.png` (480×270, embedded with `include_bytes!`; roughly 0.5 MB total).
- **Dependencies**: none new. PNG decoding is already enabled, and the lobby uses the existing `bevy_egui`.
- **Docs**: README quick start changes from "the default run is Iris" to "pick from the lobby", plus a short "adding an experience" section.
