## Context

Jackdaw (`jd` 0.19.0, Bevy 0.19, MIT/Apache-2.0) is an early-stage, standalone scene editor.

- **How a project works with it:**
  - The editor builds the game's own binary in `.jackdaw/target/game` and extracts its reflection schema.
  - Every `#[derive(Component, Reflect)]` type, from the project or its dependencies, appears in **Add Component**, with its doc comment as the tooltip. Bevy's `reflect_auto_register` (which `jackdaw_runtime` enables) registers them, so no Jackdaw-specific code is needed.
  - Scenes save as `.bsn` and load in the game through `jackdaw_runtime` (`JackdawPlugin`, `JackdawSceneRoot`).
  - Play runs the project's own binary and streams it into the editor.
- **What the spike found (2026-10-04):**
  - `jd new` writes a standalone project (its own `[workspace]`) with:
    - `bevy = "0.19"`;
    - `jackdaw_runtime = { git, rev = "5ce71373…", features = ["physics", "terrain", "pie"] }`;
    - `avian3d = "0.7"`;
    - a library `GamePlugin`, and a `main.rs` that adds `JackdawPlugin`.
  - Adding `bevaru` as a path dependency to that project, with `BevaruPlugin`, **built cleanly** (2 min 34 s, one Bevy 0.19.1).
  - `jd import` previewed on bevaru's own repository would add `jackdaw.toml`, `.jackdaw/`, and a `.gitignore` line, and asked for a root plugin name. It also showed two upstream issues (below).
- **Constraints:**
  - bevaru isn't published to crates.io.
  - `jackdaw_runtime` for Bevy 0.19 exists only on git.
  - Jackdaw is pre-1.0 and moves fast.

## Goals / Non-Goals

**Goals:**
- Users can **edit bevaru's lobby** in Jackdaw and run the result.
- Users can **reuse bevaru's visual assets** (the 3-D perceptron and the loss-shape surfaces) as components in their own Jackdaw scenes.
- bevaru gains **no Jackdaw dependency**; its build, checks, and binaries are unchanged.

**Non-Goals:**
- Changing the main `bevaru` binary's lobby. Edits apply in the Jackdaw project; that was the maintainer's choice.
- A 3-D lobby, or Jackdaw editor extensions (`.jdext`).
- Making every experience authorable. The loss-curve and experiment experiences are driven by training state and stay code-only for now; the training-objective surface can follow.
- Filing upstream issues without approval.

## Decisions

### 1. Authorable components are plain reflected data in bevaru
`src/authoring.rs` defines the components. Each is `#[derive(Component, Reflect, Default, Clone, PartialEq)]` with `#[reflect(Component, Default)]`, and a doc comment that Jackdaw shows as its tooltip.

| Component | Fields | Builds |
| --------- | ------ | ------ |
| `PerceptronDiagram` | `weights: [f64; 3]`, `bias: f64`, `activation: Activation`, `labels: bool` | `Diagram::perceptron` via `spawn_diagram`, as children |
| `LossShapeSurface` | `view: String` (a `ShapeView` id), `huber_delta`, `margin`, `resolution`, `entropy_removed` | the sampled grid as one mesh child, with the cool-to-warm colours, fitted in the standard 10 × 10 × 6 box |
| `LobbyEntry` | `experience: String`, `order: i32`, `title`, `summary`, `category: Option<String>`, `hidden: bool` | nothing; the lobby reads it |

- **Field types:** strings and enums, not handles, so the `.bsn` text is readable and the editor's inspector has simple rows. `Activation` gains `Reflect`.
- **Invalid values,** such as an unknown `view` id or a non-positive δ, log one warning naming the entity and field, and build nothing. Editing a field in the inspector must never panic the game.

*Alternative:* depend on `jackdaw_runtime` for `@EditorCategory` attributes. Rejected: a git dependency in bevaru for cosmetics. Doc comments already give tooltips.

### 2. `AuthoringPlugin` builds on add and change
- **Systems:** run on `Added<T>` or `Changed<T>`. They replace a `AuthoredChildren` child set, so edits in the inspector (live through Play) rebuild in place, and removing the component despawns its children.
- **Shared assets:** the diagram's meshes and materials come from one `DiagramAssets` resource, created on first use.
- **Explicit registration:** `register_type` is called for each type. Jackdaw's guide warns that a dependency's types can be stripped by the linker before `reflect_auto_register` runs. It costs one line each, and makes the types reliably visible.
- **Independent of the lobby:** the plugin doesn't need it, so a user's own game adds `AuthoringPlugin` alone.

### 3. Lobby overrides: entities in, layout out
- **Applying entries:** `ExperienceRegistry::lobby_layout(entries)` is a pure function. The lobby UI calls it each frame with the current `LobbyEntry` components, so there is no resource to keep in sync. It produces the categories and cards, applying order within and across categories, title, summary, category, and hidden.
  - **Unknown experience ids** are ignored, with one warning each.
  - **Ordering:** experiences without an entry keep registration order after the ordered ones.
  - **Unchanged without entries:** with no `LobbyEntry`, the layout equals `by_category`, which is today's lobby.
- **Hidden cards:** `hidden` removes the card from the lobby only. `cargo run -- <id>`, the examples, `--list`, and agents still reach the experience. Hiding is presentation, not availability.

### 4. A standalone Jackdaw project at `jackdaw/`
The spike's `jd new` output, adapted:

```text
jackdaw/
  Cargo.toml        own [workspace]; bevy 0.19; bevaru = { path = ".." };
                    jackdaw_runtime (git, pinned rev, physics + pie); avian3d 0.7
  jackdaw.toml      plugin = "GamePlugin"; [jackdaw] version 0.19.0, bevy 0.19; a Play run
  .gitignore        target/, .jackdaw/
  src/lib.rs        GamePlugin: bevaru app plugins + LobbyPlugin + AuthoringPlugin,
                    and loads assets/lobby.bsn as a JackdawSceneRoot
  src/main.rs       DefaultPlugins (via maybe_windowless), avian, JackdawPlugin, GamePlugin
  assets/lobby.bsn  one LobbyEntry entity per built-in experience
  assets/examples/perceptron.bsn, loss-shapes.bsn
  README.md
```

- **Separate from bevaru's workspace:** `jackdaw/` is listed in `workspace.exclude`, so bevaru's `cargo build`, `cargo test`, and `scripts/check.sh` never resolve Jackdaw.
- **The version pin:** the `jackdaw_runtime` rev is the one `jd 0.19.0` writes. Upgrading follows `jd upgrade jackdaw`.
- **Lobby edits:** they live in `lobby.bsn`, which loads like any authored scene. The bevaru `LobbyPlugin` sees the `LobbyEntry` entities it spawns.

*Alternative:* run `jd import --apply` on bevaru's own root.
- Rejected, for three reasons:
  - It edits bevaru's tree (`jackdaw.toml`, `.gitignore`).
  - It currently rejects bevaru's exact Bevy pin.
  - The bevaru binary can't load `.bsn` without a Jackdaw dependency.
- A separate project keeps the dependency where it's used.

### 5. Writing the `.bsn` files
- **Hand-writing them** is error-prone: the grammar belongs to `jackdaw_bsn`. So `scripts/` gains a small generator. It runs the Jackdaw project's own code to emit the initial `lobby.bsn` from the experience registry, through `jackdaw_bsn`'s writer if it is exposed, and otherwise from a checked template.
- **Validation:** a test in `jackdaw/` loads each committed `.bsn` with `jackdaw_runtime` headlessly and checks the expected components appear. It's run by the opt-in check, not by bevaru's.

### 6. Opt-in check
`scripts/check-jackdaw.sh` builds `jackdaw/` and runs its tests. It's separate from `scripts/check.sh` because it fetches git dependencies and compiles avian and the Jackdaw crates (minutes, and needs the network). `docs/jackdaw.md` says when to run it: after changing anything under `jackdaw/` or the authoring components.

### 7. Upstream findings, drafted
`docs/jackdaw.md` records the two `jd` issues the spike found, with reproduction steps and draft issue text:
- **`jd import` vs an exact Bevy pin:** an exact `bevy = "=0.19.1"` is reported as a Bevy-minor mismatch, so a project that pins its patch version needs `--allow-bevy-mismatch`.
- **`jd import`'s `cargo add` suggestion:** it suggests `cargo add jackdaw_runtime@0.19 --features physics,pie`, but crates.io has only 0.4.1 (Bevy 0.18). `jd new` uses a git revision instead.

## Risks / Trade-offs

- **[Jackdaw moves fast]** The pinned revision may stop matching a newer `jd`. → Pin it, document `jd upgrade`, and have the opt-in check catch breakage.
- **[Lobby edits don't reach the main binary]** A user may expect `cargo run` at the root to show their edits. → The docs say plainly that edits apply in `jackdaw/`. A `jackdaw` feature on bevaru is a possible follow-up.
- **[egui inside Play's windowless stream]** bevaru's UI is egui. Streaming into the editor's Game panel uses a windowless runner, and egui input there is untested. → Verify in the real editor (a task). If it fails, Play still shows the scene, and `cargo run` in `jackdaw/` is the full experience.
- **[Schema extraction misses dependency types]** → explicit `register_type` (decision 2), plus a manual check in the editor's picker.
- **[Build cost]** The `jackdaw/` project compiles avian and the Jackdaw crates. → It's opt-in and outside bevaru's workspace.

## Open Questions

- Does `jackdaw_bsn` expose a writer we can call to generate `lobby.bsn`, or do we commit a template? Decide during implementation.
- Should the training-objective surface (`loss-surface`) get an authorable component in this change, or later? Later, unless it's trivial once `LossShapeSurface` exists.
