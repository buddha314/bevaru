## 1. Plugin support

- [ ] 1.1 Add `StartupMode { Load, Idle }` (default `Load`); skip the startup load when `Idle`; test the "Idle start" scenario headlessly
- [ ] 1.2 Add an `UnloadExperiment` message that removes `Experiment`, clears `PaneViews`, resets `Playback`/`Sweep` (dropping sweep tasks), drops the pending load, and resets chart keys and weight images
- [ ] 1.3 Despawn all `SceneEntity` content on unload; make sure a reload after an unload rebuilds the scene
- [ ] 1.4 Headless tests: unloading leaves no scene entity, camera, `Experiment`, or pending task; reload after unload matches a fresh load

## 2. Experience registry

- [ ] 2.1 Add `src/experiences/mod.rs`: `Experience`, `ExperienceKind { Experiment { spec, on_loaded }, Custom }`, `OnLoaded`, `Requirement`, `Thumbnail`, `ExperienceRegistry` (category-grouped, registration order), and `RegisterExperience` for `App` (panics on duplicate ids)
- [ ] 2.2 Add `ActiveExperience`, the `in_experience(id)` run condition, the `ExperienceStarted`/`ExperienceStopped` observer events, and the `ExperienceEntity` marker with automatic despawn on stop
- [ ] 2.3 Register the built-in experiment experiences (`iris-svm`, `regression-mse-vs-mae`, `loss-curves`, `mnist-svm`) in `ExperiencesPlugin`, moving their specs and on-load actions out of the examples; gate MNIST on the feature with an enable hint
- [ ] 2.4 Move the sigmoid display from `examples/ml_interactive.rs` into `src/experiences/sigmoid.rs` as the `sigmoid` custom experience (camera and sprite marked `ExperienceEntity`; refresh only while active)
- [ ] 2.5 Tests: built-in ids present, unique, kebab-case, and categorized; a duplicate registration panics naming the id; a third-party registration appears in category order; every available experiment experience builds; sigmoid systems don't run when inactive

## 3. Lobby

- [ ] 3.1 Add the `AppScreen { Lobby, Loading, Running }` state and `LobbyPlugin` (inserts `StartupMode::Idle`); wire experiment start (`Loading` → `Running` on arrival, → `Lobby` with a per-card error on failure) and custom start (→ `Running` + `ExperienceStarted`)
- [ ] 3.2 Run `on_loaded` (play or sweep) exactly once on entering `Running`
- [ ] 3.3 Build the egui lobby: category headings, a wrapped card grid with thumbnail, title, summary, requirement note, disabled state, and last error
- [ ] 3.4 Hide the control and chart panels and zero `UiInsets` outside `Running`; show a loading indicator in `Loading`
- [ ] 3.5 Add "◀ Lobby" to the control panel (experiment kind) and as an overlay (custom kind); `Esc` does the same unless egui has keyboard focus. On leaving: `ExperienceStopped`, `UnloadExperiment`, despawn `ExperienceEntity`, clear `ActiveExperience`
- [ ] 3.6 Headless tests: the lobby stays idle for 60 frames; choosing an experiment experience loads it and runs its action; leaving tears it down; the regression → lobby → sigmoid round trip leaves only the sigmoid

## 4. Clean-up on leave

- [ ] 4.1 Route every exit (Back, `Esc`, load failure, switch, quit) through one `leave_experience` that triggers `ExperienceStopped`, sends `UnloadExperiment`, despawns `ExperienceEntity` and `SceneEntity`, clears `ActiveExperience`, and bumps a session counter
- [ ] 4.2 Tag experiment loads, sweep tasks, and chart renders with the session counter; discard results from older sessions on arrival
- [ ] 4.3 Remove experiment-specific image assets held in resources (MNIST weight images); reset the shared chart images to blank instead of re-adding them
- [ ] 4.4 Replace the control panel's grow-only egui texture map with a resource that releases textures via `remove_image` when their images go away
- [ ] 4.5 Remove the sigmoid experience's resources (refresh timer) in its `ExperienceStopped` handler
- [ ] 4.6 On `AppExit`, call `leave_experience` before shutdown; in the MNIST loader, delete stale `*.partial` files before downloading (with a test)
- [ ] 4.7 Headless leak test: enter and leave every available experience 10 times; entity, mesh, material, image, and egui texture counts and task queues match the lobby baseline. Also test leaving during a load and during a sweep or chart render

## 5. Thumbnails

- [ ] 5.1 Add a `--thumbnail <path>` dev mode that captures only the scene viewport (pane rect) after a delay and exits
- [ ] 5.2 Add `scripts/thumbnails.sh`, which captures every registered id and writes 480×270 PNGs to `assets/thumbnails/`; generate thumbnails for the five built-ins
- [ ] 5.3 Embed built-in thumbnails with `include_bytes!`; decode them once into egui textures; draw a titled placeholder when an experience has no thumbnail; support `Thumbnail::Asset` through the `AssetServer`

## 6. Binary, examples, and command line

- [ ] 6.1 Add `parse_args` (no args → lobby, `--list`, known id, unknown id → error) with unit tests
- [ ] 6.2 Rewrite `src/main.rs` as the lobby app; `--list` prints ids and exits 0; unknown ids print valid ids to stderr and exit non-zero without opening a window; a known id starts directly in that experience
- [ ] 6.3 Reduce every `examples/*.rs`, including `ml_interactive`, to `shared::run_experience(<id>)`

## 7. Docs and verification

- [ ] 7.1 Update the README: `cargo run` opens the lobby; experience ids; `cargo run -- <id>`; `Esc` / "Back to lobby"; an "adding an experience" section (experiment and custom kinds, `ExperienceEntity`, thumbnails and the regeneration script)
- [ ] 7.2 Run the binary: screenshot the lobby; open each available experience, return with `Esc`, and confirm a clean scene each time, including the sigmoid round trip; watch memory over repeated round trips in a release build
