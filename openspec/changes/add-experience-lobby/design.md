## Context

The `bevaru` binary (`src/main.rs`, the `default-run` target) just calls `examples/iris_svm.rs`'s `main`.
- The other experiences (`regression_mse_vs_mae`, `loss_curves`, `mnist_svm`) are separate `--example` binaries. Each builds its own `App` with a hard-coded `StartupExperiment` and a one-shot "on loaded" system (play, or start a sweep).
- `ml_interactive` is different in kind: no experiment, just a ruviz sigmoid plot shown as a sprite with its own `Camera2d`.

The plugin is close to supporting a lobby, but not quite:
- **Always loads at startup:** `ExperimentPlugin` always requests an experiment at `Startup`. If no `StartupExperiment` is set, it falls back to `ExperimentSpec::default()`.
- **Switching works:** `LoadExperiment` already swaps experiments. A new `Experiment` resource replaces the old one, and the scene rebuilds when its `generation` changes.
- **No way to have nothing loaded:** nothing supports having *no* experiment after one has loaded. Scene systems are gated on `resource_exists::<Experiment>`, so removing the resource stops them, but the entities they spawned would stay. Sweep tasks and in-flight loads also have no cancel path besides being replaced.

Issue #12 asks for a lobby. The maintainer has confirmed four things:
- lobby cards get **thumbnails**;
- the **sigmoid** plot belongs in the lobby;
- **more experiences are coming soon**, so they need a registry;
- **no** "continue last experience".

## Goals / Non-Goals

**Goals:**
- One `cargo run` shows everything bevaru can do. Pick an experience, explore it, come back, pick another.
- **Easy to add more:** adding an experience is one `register_experience` call (plus a plugin for custom ones). The lobby, `--example` binaries, and command-line deep links pick it up automatically.
- **Two kinds:** experiences can be ML experiments (standard scene and controls) or custom (anything else, like the sigmoid plot).
- **Clean exit:** leaving an experience is a clean teardown; nothing from it survives into the next one.

**Non-Goals:**
- "Continue" / remembering the last experience between runs (declined).
- User-authored experiences, or loading experience definitions from data files (RON/TOML).
- Live animated previews on cards. Thumbnails are static images.
- Search, filtering, or favourites in the lobby. Category grouping is enough for the planned number of experiences; revisit past roughly 20.

## Decisions

### 1. A registry resource with an `App` extension trait
```rust
pub struct Experience {
    pub id: &'static str,          // kebab-case, unique
    pub title: &'static str,
    pub summary: &'static str,     // one sentence: what it teaches
    pub category: &'static str,    // lobby heading
    pub thumbnail: Option<Thumbnail>,
    pub requires: Requirement,     // None | Feature { name, hint }
    pub kind: ExperienceKind,
}
pub enum ExperienceKind {
    Experiment { spec: fn() -> ExperimentSpec, on_loaded: OnLoaded }, // Nothing | Play | Sweep(SweepSpec)
    Custom,
}
pub trait RegisterExperience { fn register_experience(&mut self, e: Experience) -> &mut Self; }
#[derive(Resource)] pub struct ExperienceRegistry { /* ordered entries */ }
```
- **Registration:** `ExperiencesPlugin` registers the five built-ins. Any plugin can call `app.register_experience(...)` in `build`.
- **Duplicates:** a duplicate id panics at `build` time with the id in the message. This is a programming error, caught the first time the app starts, and the spec's "Duplicate id" scenario tests it with `#[should_panic]`.
- **Ordering:** category headings appear in the order their first entry was registered, and entries keep registration order within a category. This is deterministic and needs no sort keys.
- **Entries that need a missing feature** still register, so the lobby can show them disabled. For example, MNIST's `spec` function is only meaningful under `cfg(feature = "mnist")`; without it the entry exists but `is_available()` is false.
- **Why a resource, not the static slice from the first draft**: experiences are about to multiply and may live in other crates. A static slice can't be extended from outside, and the examples would have to know about every crate. The `App` extension follows Bevy's own registration style (`add_message`, `init_gizmo_group`).
- **Why `&'static str` and `fn` pointers**: entries are compile-time definitions. Keeping them `'static` and `Copy`-friendly means no allocation and easy tests. A third-party crate can still define its entries as statics.

### 2. Custom experiences: scoped systems, lifecycle events, auto-despawned entities
A custom experience's plugin uses four pieces:
- **Run condition:** `in_experience("sigmoid")` gates its systems; it checks `ActiveExperience`.
- **Lifecycle events:** `ExperienceStarted { id }` and `ExperienceStopped { id }`, delivered to observers (`app.add_observer(...)`), for setup and teardown.
- **Ownership marker:** spawned entities carry an `ExperienceEntity` component. Everything carrying it is despawned when the experience stops, even if the plugin's own teardown forgets something.
- **Back button:** the lobby draws a small "◀ Lobby" overlay for custom experiences. Experiment experiences get the button inside the control panel instead.

The sigmoid plot moves from `examples/ml_interactive.rs` into `src/experiences/sigmoid.rs`:
- On `ExperienceStarted`, it spawns a `Camera2d` and the plot sprite, both marked `ExperienceEntity`.
- Its refresh timer runs only `in_experience("sigmoid")`.
- `SigmoidPlotPlugin` (the original scaffold) is unchanged; the experience just displays its `PlotPngBytes`.
- `ml_interactive` becomes a wrapper that starts `sigmoid`.

- **Why**: observers and run conditions are the standard Bevy way to scope behaviour, and the marker gives a safety net for teardown. **Alternative**: each custom experience is its own Bevy sub-app. That gives stronger isolation but is far heavier, and sub-apps can't share the window and egui context easily.

### 3. Screen state
`#[derive(States)] pub enum AppScreen { #[default] Lobby, Loading, Running }`, plus `ActiveExperience(Option<&'static str>)`. Transitions:
- **Card activated, experiment kind:** set `ActiveExperience`, go to `Loading`. `OnEnter(Loading)` sends `LoadExperiment`. When the `Experiment` arrives, go to `Running` and run `on_loaded` once. A load error goes back to `Lobby`, with the error stored against the card.
- **Card activated, custom kind:** set `ActiveExperience`, go straight to `Running`, trigger `ExperienceStarted`.
- **Leaving:** `OnEnter(Lobby)` triggers `ExperienceStopped` for the active id, sends `UnloadExperiment` (decision 5), despawns all `ExperienceEntity`, and clears `ActiveExperience`.

`StartupMode::Idle` (decision 4) keeps the plugin from loading anything until a card is picked.

### 4. Idle start is opt-in on the existing plugin
`pub enum StartupMode { Load, Idle }` is a resource, defaulting to `Load` (today's behaviour). `LobbyPlugin` inserts `Idle`; existing apps are untouched (spec: modified "Bevy plugin entry point").
- **Alternative**: change the default to idle. That would silently break every existing app.

### 5. Experiment teardown is a library feature
An `UnloadExperiment` message removes everything an experiment created:
- the `Experiment` resource;
- the contents of `PaneViews`;
- `Playback` and `Sweep` state (resetting `Sweep` drops its tasks, which cancels them);
- the pending `ExperimentLoader` task;
- every `SceneEntity` (pane cameras and lights included);
- the charts' shown/wanted keys and weight images.

`rebuild_scene` keys on `(generation, pane count)`, and generation keeps increasing, so a reload always rebuilds.
- **Risk**: something added later forgets to register its teardown. Mitigation: a headless test loads, unloads, and asserts that no `SceneEntity`, pane camera, `Experiment` or pending task remains.

### 6. Clean-up on leave is measured, not assumed
Leaving must return the app to its lobby baseline (spec: "Clean-up on leave"). The approach:
- **One exit path:** every way out (Back, `Esc`, load failure, switching, quitting) goes through the same `leave_experience` function. It triggers `ExperienceStopped`, sends `UnloadExperiment`, despawns `ExperienceEntity` and `SceneEntity`, clears `ActiveExperience`, and bumps a **session counter**.
- **Late results:** async work captures the session counter when spawned, and any result from an older session is discarded on arrival. This covers experiment loads, sweep solutions, and chart renders, which can't be interrupted once their synchronous work has started.
- **Assets:** per-experiment meshes, materials, and region images are owned only by entity components, so despawning frees them. Experiment-specific images held in resources are removed explicitly: today that's the MNIST weight images in `WeightImages`. The shared chart images are kept and reset to blank, so their count stays constant.
- **egui textures:** the control panel's texture map currently only grows (`src/controls.rs`). It becomes a resource that releases every texture with `EguiContexts::remove_image` when its image goes away, keeping only the long-lived chart and thumbnail textures.
- **Quit:** an `AppExit` observer calls `leave_experience` before shutdown.
- **Partial downloads:** the MNIST loader deletes stale `*.partial` files before downloading.
- **Proof:** a headless test enters and leaves every available experience ten times. It compares entity, `Assets<Mesh>`, `Assets<StandardMaterial>`, `Assets<Image>`, and egui texture counts against the lobby baseline, and asserts that sweep, load, and chart-render tasks are empty.

### 7. Thumbnails: dedicated PNGs, embedded
- **Image:** each built-in experience has `assets/thumbnails/<id>.png`, 480×270, cropped to the scene area, embedded with `include_bytes!`.
- **Loading:** the first time the lobby shows, the bytes are decoded into a Bevy `Image` via `Image::from_buffer` and registered as egui textures once.
- **Third-party thumbnails:** `Thumbnail::Embedded(&'static [u8])` or `Thumbnail::Asset(&'static str)`, loaded through the `AssetServer`. `None` gets a placeholder with the title on a tinted card.
- **Why PNG, not reusing the README WebPs**: PNG decoding is already enabled through Bevy's `2d`/`3d` features, so this needs no new feature or dependency. The README images are full-window captures (panels included), which make poor 480 px thumbnails; a scene-only crop reads much better.
- **Why embedded**: the spec requires thumbnails to work from any working directory, and `include_bytes!` makes the binary self-contained. The cost is about 50–120 KB per image.
- **Regeneration:** the existing `DevScreenshot` support, plus a `--thumbnail` mode that captures only the scene viewport. That mode reuses the pane rect, so it doesn't depend on panel widths. A script (`scripts/thumbnails.sh`) runs every registered id and writes `assets/thumbnails/`. It is documented in the README.

### 8. Lobby UI in egui
- **Layout:** an `egui::CentralPanel` with category headings, each followed by a responsive wrapped grid of cards.
- **Card contents:** thumbnail, title, summary, requirement note, and the last error if a load failed.
- **States:** a disabled card is drawn dimmed and is not clickable.
- **Panels:** while not `Running`, the control and chart panels are hidden and `UiInsets` is zeroed.
- **`Esc`:** returns to the lobby unless `EguiWantsInput::wants_any_keyboard_input()`.
- **Why**: reuses the existing UI camera, fonts, and theming. A Bevy-UI lobby would add a second UI system.

### 9. Command line before the `App` is built
`src/main.rs` parses arguments with a pure `fn parse_args(args, registry) -> Result<Launch, String>`, unit-tested without a window, and with no clap dependency:
- `--list` prints the ids and exits with code 0;
- an unknown id prints the ids to stderr and exits with code 2 without opening a window;
- a known id starts the app directly in that experience.

Building the registry needs an `App`. `main` builds the app first, reads the registry from it, then either runs or exits. The window only opens when `run()` is called, so exiting early never shows one.

### 10. Examples become one-liners
Each example becomes `fn main() { shared::run_experience("iris-svm") }`. `run_experience` builds the windowed app with `LobbyPlugin` configured to start directly in that id, so `cargo run --example` behaves as before, and "Back to lobby" now works there too.

## Risks / Trade-offs

- **Leaving mid-load leaves a task computing in the background** → dropping the `Task` cancels it at the next await point. `Experiment::build` runs synchronously inside the task, so an MNIST PCA already running finishes and its result is discarded: a second or two of wasted CPU, never shown.
- **A custom experience spawns entities without `ExperienceEntity`, or inserts resources it doesn't remove** → they would leak into the lobby. Mitigation: the ten-round-trip test catches entity, asset, and texture growth for every registered experience, including third-party ones registered in the test app. The README's "adding an experience" section shows the marker and the stop handler.
- **Thumbnails go stale as visuals change** → they're regenerated by one command, and a missing or stale thumbnail never breaks anything.
- **The MNIST card fails offline** → the error shows on the card, including the `BEVARU_MNIST_DIR` hint.
- **The lobby and the control panel both want the central egui area** → they're mutually exclusive by state, and a test asserts the control system doesn't run in `Lobby`.

## Migration Plan

Additive. The default `StartupMode::Load` keeps every existing app's behaviour.
- `cargo run` changes from "Iris sweep" to "lobby"; `cargo run -- iris-svm` restores the old one-step start.
- `cargo run --example <name>` still opens its experience directly.

## Open Questions

None blocking. Resolved with the maintainer: thumbnails yes; sigmoid in the lobby; a registry for upcoming experiences; no "continue".
