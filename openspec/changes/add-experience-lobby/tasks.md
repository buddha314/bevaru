## 1. Plugin support

- [ ] 1.1 Add `StartupMode { Load, Idle }` (default `Load`); skip the startup load when `Idle`; test the "Idle start" scenario headlessly
- [ ] 1.2 Add an `UnloadExperiment` message that removes `Experiment`, clears `PaneViews`, resets `Playback`/`Sweep` (dropping sweep tasks), drops the pending load, and resets chart keys
- [ ] 1.3 Despawn all `SceneEntity` content (pane cameras, lights, markers, planes, region quads) on unload; make sure a reload after an unload rebuilds the scene
- [ ] 1.4 Headless tests: unloading leaves no scene entity, camera, `Experiment`, or pending task; reload after unload matches a fresh load

## 2. Experience catalogue

- [ ] 2.1 Add `src/experiences.rs` with `Experience`, `OnLoaded`, `Requirement`, and `EXPERIENCES` holding `iris-svm`, `regression-mse-vs-mae`, `loss-curves`, `mnist-svm`, moving the specs and on-load actions out of the examples
- [ ] 2.2 Add `is_available()` plus a human-readable reason for gated entries (MNIST without the feature)
- [ ] 2.3 Tests: ids are unique and kebab-case and the four required ids are present; every available experience's spec builds via `Experiment::build`
- [ ] 2.4 Add `shared::run_experience(id)` and reduce `iris_svm`, `regression_mse_vs_mae`, `loss_curves`, and `mnist_svm` examples to calls to it

## 3. Lobby

- [ ] 3.1 Add the `AppScreen { Lobby, Loading, Running }` state and `LobbyPlugin` (inserts `StartupMode::Idle`); wire `Loading → Running` when the experiment arrives and `→ Lobby` on load error, recording the error per card
- [ ] 3.2 Run `on_loaded` (play or sweep) exactly once on entering `Running`
- [ ] 3.3 Build the egui lobby: card grid with title, description, requirement note, disabled state for unavailable entries, and the last error
- [ ] 3.4 Hide the control and chart panels and zero `UiInsets` outside `Running`; show a loading indicator in `Loading`
- [ ] 3.5 Add a "◀ Lobby" button to the control panel and `Esc` (ignored while egui has keyboard focus); both send `UnloadExperiment` and go to `Lobby`
- [ ] 3.6 Headless tests: the lobby stays idle for 60 frames; choosing an experience loads it and runs its action; leaving tears it down; a round trip shows only the second experience

## 4. Binary and command line

- [ ] 4.1 Add `parse_args` (no args → lobby, `--list`, known id, unknown id → error) with unit tests
- [ ] 4.2 Rewrite `src/main.rs` as the lobby app; `--list` prints ids and exits 0; unknown ids print valid ids to stderr and exit non-zero without opening a window; a known id starts in `Loading`

## 5. Docs and verification

- [ ] 5.1 Update the README: `cargo run` opens the lobby, the experience ids, `cargo run -- <id>`, and `Esc` / "Back to lobby"
- [ ] 5.2 Run the binary: screenshot the lobby; open each available experience, return with `Esc`, and confirm a clean scene each time
