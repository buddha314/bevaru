## Context

The `bevaru` binary (`src/main.rs`, the `default-run` target) just calls `examples/iris_svm.rs`'s `main`. The other experiences (`regression_mse_vs_mae`, `loss_curves`, `mnist_svm`) are separate `--example` binaries, each building its own `App` with a hard-coded `StartupExperiment` and a one-shot "on loaded" system (play, or start a sweep).

The plugin is close to supporting a lobby, but not quite:
- `ExperimentPlugin` always requests an experiment at `Startup`. If no `StartupExperiment` is set, it falls back to `ExperimentSpec::default()`.
- Switching experiments already works via `LoadExperiment`: a new `Experiment` resource replaces the old one, and the scene rebuilds when its `generation` changes.
- Nothing supports having *no* experiment after one has loaded. Scene systems are gated on `resource_exists::<Experiment>`, so removing the resource stops them, but the entities they spawned would stay. Sweep tasks and in-flight loads also have no cancel path besides being replaced.

Issue #12 asks for a lobby where users pick an experience.

## Goals / Non-Goals

**Goals:**
- One `cargo run` shows everything bevaru can do. Pick an experience, explore it, come back, pick another.
- Each experience is defined exactly once and shared by the lobby, the `--example` binaries, and command-line deep links.
- Leaving an experience is a clean teardown: nothing from it survives into the next one.

**Non-Goals:**
- User-authored or saved experiences, and persisting the last choice between runs.
- Thumbnails or animated previews on lobby cards. These are a nice follow-up: the README screenshots exist, but showing WebP needs an extra Bevy image feature. See Open Questions.
- The `ml_interactive` sigmoid scaffold. It is a different kind of app (a sprite, no experiment) and stays an `--example` only.
- Moving experience definitions into data files (RON/TOML). The catalogue is Rust code for now.

## Decisions

### 1. Catalogue as a static slice of plain structs in the library
`src/experiences.rs` defines `pub struct Experience { id, title, summary, requires: Requirement, spec: fn() -> ExperimentSpec, on_loaded: OnLoaded }` and `pub static EXPERIENCES: &[Experience]`.
- `OnLoaded` is one of `Nothing`, `Play`, or `Sweep(SweepSpec)`.
- `Requirement` is `None` or `Feature("mnist")`. Entries that need an unbuilt feature still compile and are listed, so the lobby can show them disabled. The spec function only builds the real MNIST spec under `cfg(feature = "mnist")`; otherwise the entry is never loadable.

- **Why**: the examples and the lobby must not drift (spec: "Example and lobby agree"). Plain data with `fn` pointers is `'static`, trivially testable ("every available experience loads"), and needs no registration machinery.
- **Alternative**: an `ExperienceRegistry` resource that plugins add to at build time. More extensible for third-party experiences, but nothing needs that yet. It can wrap the static slice later without changing entries.

### 2. A Bevy state for the screen, not ad-hoc flags
`#[derive(States)] pub enum AppScreen { #[default] Lobby, Loading, Running }`, owned by a new `LobbyPlugin`.
- `OnEnter(Loading)` sends `LoadExperiment`.
- The existing loader inserting `Experiment` moves the state to `Running` and fires `on_loaded`.
- A load error moves back to `Lobby` and records the error against the card.
- `OnEnter(Lobby)` runs teardown (decision 4).
- **Why**: state transitions give exactly-once enter/exit hooks for teardown and load. Run conditions like `in_state(AppScreen::Running)` keep the lobby idle (spec: "Lobby is idle").
- **Alternative**: keep using `resource_exists::<Experiment>` alone. That can't tell "loading" from "lobby", and teardown would have to be inferred from a resource disappearing.

### 3. Idle start is opt-in on the existing plugin
Add `pub enum StartupMode { Load, Idle }` as a resource, defaulting to `Load` (today's behaviour). `request_startup_experiment` returns early when it is `Idle`. `LobbyPlugin` inserts `Idle`; examples and existing apps are untouched.
- **Why**: the modified `visualization-scene` requirement keeps current behaviour as the default. Only the lobby opts out.
- **Alternative**: change the default to idle. That would silently break every existing app that relies on the default experiment.

### 4. Teardown is a library function, not lobby-specific
Add `UnloadExperiment` (a message) handled in the plugin. It:
- removes the `Experiment` resource;
- clears `PaneViews`, resets `Playback` and `Sweep` (dropping its tasks cancels them);
- drops `ExperimentLoader`'s pending task;
- despawns everything tagged `SceneEntity` (which already includes pane cameras and lights);
- resets the charts' shown/wanted keys so the next experiment re-renders from scratch.

The scene's `rebuild_scene` keys on `(generation, pane count)`. Generation keeps increasing across loads, so a reload after an unload always rebuilds.
- **Why**: the spec's "Experiment teardown" requirement belongs to `visualization-scene`, not the lobby. Any app that switches content benefits.
- **Risk**: something added later that forgets to register its teardown. Mitigation: one headless test loads an experiment, unloads it, and asserts that no `SceneEntity`, pane camera, `Experiment` or pending task remains.

### 5. Lobby UI in egui, in the existing UI camera
The lobby is an `egui::CentralPanel` showing a responsive grid of cards: title, summary, a requirement note, a disabled state, and the last error. While `Running`, the control panel gets a "◀ Lobby" button at its top. `Esc` returns to the lobby unless `EguiWantsInput::wants_any_keyboard_input()` (spec: "Esc while typing does not leave").
- **Why**: the controls already use egui and have a dedicated full-window UI camera, so the lobby gets layout, fonts and theming for free. A Bevy-UI lobby would introduce a second UI system.
- During `Lobby` and `Loading`, the control and chart panels are hidden, and `UiInsets` is zeroed so no stale viewport math runs.

### 6. Command line handled before the `App` is built
`src/main.rs` parses `std::env::args()` itself, with no clap dependency for two forms:
- `--list` prints the ids and exits with code 0;
- an unknown id prints the ids to stderr and exits with code 2 without opening a window;
- a known id starts the app in `Loading` for that experience.

The parsing lives in a pure function (`fn parse_args(&[String]) -> Result<Launch, String>`) so it can be unit-tested without a window.

### 7. Examples become one-liners
`examples/iris_svm.rs` and the others become: `fn main() { shared::run_experience("iris-svm") }`. `run_experience` builds the window app with `BevaruPlugin` and a `StartupExperiment` from the catalogue entry, and runs its `on_loaded`. There is no lobby there, so `cargo run --example` behaves exactly as before. `src/main.rs` no longer includes an example file by path.

## Risks / Trade-offs

- [Leaving mid-load leaves a task computing in the background] → dropping the `Task` cancels it at its next await point. `Experiment::build` is synchronous inside the task, so an MNIST PCA already running finishes and its result is discarded. Acceptable: a second or two of CPU, never shown.
- [The MNIST card fails offline] → the error is shown on the card (spec "Load failure returns to the lobby"), with the `BEVARU_MNIST_DIR` hint.
- [The lobby and the control panel both want the egui central area] → they are mutually exclusive by state, and a test asserts the controls system doesn't run in `Lobby`.
- [Catalogue entries and README drift] → README lists ids generated by hand. Acceptable, but the `--list` output is what users can rely on.

## Migration Plan

Additive. The default `StartupMode::Load` keeps every existing app's behaviour. `cargo run` changes from "Iris sweep" to "lobby"; `cargo run -- iris-svm` restores the old one-step start. The README is updated accordingly.

## Open Questions

- Card thumbnails: embed the README screenshots (needs Bevy's `webp` feature, or re-encoding to PNG, ~50–140 KB each) or render live miniature previews later?
- Should the lobby remember the last experience and offer "Continue"? Out of scope here unless wanted.
- Should `ml_interactive` appear in the lobby as a "sigmoid sketch" entry after all? It needs a different screen type, not an experiment.
