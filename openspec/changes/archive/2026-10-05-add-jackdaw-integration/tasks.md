## 1. Authorable components (`src/authoring.rs`)

- [x] 1.1 `PerceptronDiagram`, `LossShapeSurface`, and `LobbyEntry`: `Component + Reflect + Default`, `#[reflect(Component, Default)]`, with doc comments for tooltips; `Activation` gains `Reflect`
- [x] 1.2 `AuthoringPlugin`: explicit `register_type` for each; build-on-`Added`/`Changed` systems that replace an `AuthoredChildren` set; despawn on removal; shared `DiagramAssets` on first use
- [x] 1.3 `LossShapeSurface` builds one mesh child from `ShapeView::sample` + `grid_mesh` (coolwarm, fitted box); invalid ids or values log one warning naming the entity and field, and build nothing
- [x] 1.4 Tests (headless): build on add (7 nodes, 6 edges); rebuild on change (negative weight takes the negative colour, no stale children); removal cleans up; unknown view warns without panicking; registry contains all three types; standalone use without the lobby

## 2. Lobby layout overrides

- [x] 2.1 `ExperienceRegistry::lobby_layout(entries)`: a pure function the lobby calls with the current `LobbyEntry` components; unknown ids warn once
- [x] 2.2 The lobby UI applies order, title, summary, category, and hidden; experiences without entries follow in registration order; empty categories disappear
- [x] 2.3 Hidden experiences stay startable by id, in the examples, by remote control, and in `--list`
- [x] 2.4 Tests: reorder and retitle; move category; hidden but startable; no entries means an unchanged lobby; unknown id ignored

## 3. The Jackdaw project (`jackdaw/`)

- [x] 3.1 Add `jackdaw` to `workspace.exclude`; create `Cargo.toml` (own `[workspace]`, `bevy = "0.19"`, `bevaru = { path = ".." }`, `jackdaw_runtime` at the `jd 0.19.0` rev with `physics` and `pie`, `avian3d = "0.7"`), `jackdaw.toml` (`plugin = "GamePlugin"`, version pins, a Play run), and `.gitignore`
- [x] 3.2 `src/lib.rs` `GamePlugin` (bevaru app plugins, `LobbyPlugin`, `AuthoringPlugin`, load `assets/lobby.bsn`) and `src/main.rs` (`maybe_windowless(DefaultPlugins)`, avian, `JackdawPlugin`, `GamePlugin`)
- [x] 3.3 `assets/lobby.bsn` with one `LobbyEntry` per built-in experience, generated from the registry (via `jackdaw_bsn`'s writer if exposed, else a checked template); `assets/examples/perceptron.bsn` and `loss-shapes.bsn`
- [x] 3.4 Tests in `jackdaw/`: every committed `.bsn` loads headlessly with the expected components; the lobby scene yields one entry per built-in experience
- [x] 3.5 `scripts/check-jackdaw.sh`: builds `jackdaw/` and runs its tests; confirm `scripts/check.sh` resolves no Jackdaw crate

## 4. Docs

- [x] 4.1 `docs/jackdaw.md`: open and edit the lobby; Play and `cargo run`; use the components in your own project (bevaru as a git dependency, add `AuthoringPlugin`); edits apply to `jackdaw/` only; upgrading the pin; the two upstream findings with reproductions and draft issues (not filed)
- [x] 4.2 README: a Jackdaw section linking the doc; regenerate `docs/agents/capabilities.*` if the manifest changes

## 5. Verification

- [x] 5.1 `scripts/check.sh --tests` and `scripts/check-jackdaw.sh` pass
- [x] 5.2 In the real editor: `jd open jackdaw`; the three components appear in Add Component with their tooltips; edit `lobby.bsn` (reorder, retitle, hide) and see it in Play and in `cargo run`; drop a `PerceptronDiagram` and a `LossShapeSurface` into a scene and edit their fields live; note how bevaru's egui UI behaves in Play's stream
