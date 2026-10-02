## 1. Workspace Setup

- [x] 1.1 Upgrade the root crate from Bevy 0.14 to current stable (and ruviz if required); keep the scaffold's two tests green
- [x] 1.2 Convert the repo to a Cargo workspace: root `bevaru` stays the plugin, add `crates/bevaru-core` (no Bevy)
- [x] 1.3 Pin exact `bevy` and matching `bevy_egui`; add `nalgebra`, `rand`/`rand_chacha` (seeded RNG) to core
- [x] 1.4 Add `mnist` cargo feature gating `ureq`, `sha2`, `dirs`
- [x] 1.5 Add a GitHub Actions workflow (none exists yet): `cargo test -p bevaru-core`, `cargo build --examples`, `cargo build` with and without `mnist`, `cargo tree -p bevaru-core` asserting no `bevy`

## 2. Loss Functions (bevaru-core)

- [x] 2.1 Define `LossKind` enum (MSE, MAE, Huber, Hinge, SquaredHinge, Logistic, ZeroOne) with `ALL`, display names, and `task()`
- [x] 2.2 Define `LossParams` with validated constructors (Huber `δ > 0`, margin `> 0`) returning typed errors
- [x] 2.3 Implement pointwise `value` and `grad` for regression losses over residual
- [x] 2.4 Implement pointwise `value` and `grad` for classification losses over margin (numerically stable logistic via `ln_1p`/softplus)
- [x] 2.5 Implement batch mean loss with optional L2 regularization excluding bias
- [x] 2.6 Unit tests for every spec scenario value, plus a finite-difference gradient check over all differentiable losses

## 3. Datasets (bevaru-core)

- [x] 3.1 Define `Dataset` (features, targets, feature names, class names) and `Target` (class labels / real values)
- [x] 3.2 Embed Iris CSV with `include_str!` and parse; test 150×4, 3×50
- [x] 3.3 Implement binary task derivation (class-vs-class, one-vs-rest) mapping labels to ±1
- [x] 3.4 Implement seeded synthetic generators: separable blobs, overlapping blobs, regression with outlier fraction
- [x] 3.5 Implement feature-column projection and PCA (symmetric eigendecomposition of covariance, explained variance)
- [x] 3.6 Implement MNIST loader: IDX parsing, mirror list, SHA-256 verification, cache dir, `BEVARU_MNIST_DIR` override, seeded subsampling
- [x] 3.7 Tests: checksum-mismatch rejection (with a corrupted fixture), cached reload makes no network call, PCA orthonormality

## 4. Linear Models and Trainer (bevaru-core)

- [x] 4.1 Implement `LinearModel { w, b }` with `predict` and `decision_function`
- [x] 4.2 Implement `TrainerConfig` (model kind, loss, `λ`/`C`, learning rate + schedule, batch size, budget, tolerance, seed) with loss/task compatibility validation
- [x] 4.3 Implement `Trainer::step()` for regression, SVM (primal hinge / squared hinge, Pegasos-style schedule), and logistic regression
- [x] 4.4 Record `Snapshot`s (step, w, b, loss, support vectors) with capped, decimated history; implement `snapshot(i)`
- [x] 4.5 Implement convergence detection and `BudgetExhausted`; detect non-finite parameters as `Diverged`
- [x] 4.6 Tests: determinism under seed, loss decrease on separable data, C-sweep margin/‖w‖ ordering, support vectors lie within margin, 784-D training

## 5. Visualization Scene (bevaru)

- [x] 5.1 Generalize `PlotPngBytes`/`RefreshPlotEvent` to per-chart ruviz renders on `AsyncComputeTaskPool`, decoded into Bevy `Image` textures and displayed on screen; keep the existing headless behaviour *(as built: ruviz renders raw RGBA, no PNG decode; the sigmoid scaffold is kept as `SigmoidPlotPlugin`)*
- [x] 5.2 Split `BevaruPlugin` into sub-plugins (`ScenePlugin`, `ChartsPlugin`, `ControlsPlugin`, `PlaybackPlugin`); define core resources/components (`ActiveDataset`, `ActiveTrainer`, `DisplayedModel`) *(as built: resources are `Experiment`, `PaneViews`, `Playback`, `Sweep`; `ExperimentPlugin` added)*
- [x] 5.3 Render data points with instanced meshes, Okabe–Ito palette, per-class marker shapes, and a legend
- [x] 5.4 Render 2D decision line clipped to data bounds, with zero-`w` guard and UI message
- [x] 5.5 Render SVM margin lines and support-vector highlight rings
- [x] 5.6 Implement decision-region shading material (uniforms `w`, `b`, confidence mode) with a toggle *(as built: CPU-computed region texture instead of a shader material — see design §8)*
- [x] 5.7 Render regression fit line and residual segments with a toggle
- [x] 5.8 Render 3D hyperplane as a clipped translucent quad for 3-feature projections
- [x] 5.9 Implement 2D pan/zoom, 3D orbit camera, and "frame data"
- [x] 5.10 Implement loss-curve chart (overlay selected losses vs residual/margin with current hyperparameters) via ruviz
- [x] 5.11 Implement live training-loss plot with current-step marker
- [x] 5.12 Implement MNIST weight-vector 28×28 image view and "projection" labelling for high-dimensional boundaries

## 6. Animation and Interaction (bevaru)

- [x] 6.1 Implement playback state (play/pause/step/speed) driving `Trainer::step()` from a fixed-timestep system *(as built: `Update` with a step accumulator and an 8 ms per-frame training budget)*
- [x] 6.2 Implement timeline scrubbing from snapshot history
- [x] 6.3 Implement boundary interpolation by (unit normal slerp, offset lerp, margin lerp) with configurable duration *(as built: exponential approach over `transition_secs`, geometric scale interpolation)*
- [x] 6.4 Implement hyperparameter sweep (linear/log range, sample count) computed on `AsyncComputeTaskPool` with progress display and streamed results
- [x] 6.5 Implement live loss switching (restart from same init) and live slider edits (continue from current parameters)
- [x] 6.6 Build the egui control panel: dataset/task, projection, model/loss (filtered by task), hyperparameter sliders (log where appropriate), playback, sweep config, visibility toggles
- [x] 6.7 Implement side-by-side comparison mode with two trainers and synchronized playback

## 7. Examples and Docs

- [x] 7.1 `examples/loss_curves.rs`: all losses overlaid, with sliders for `δ` and margin
- [x] 7.2 `examples/regression_mse_vs_mae.rs`: outlier data, MSE vs MAE vs Huber in comparison mode
- [x] 7.3 `examples/iris_svm.rs`: binary Iris task on petal features, animated `C` sweep, hinge vs logistic
- [x] 7.4 `examples/mnist_svm.rs` (requires `mnist`): 3-vs-8 linear SVM, PCA view plus weight image
- [x] 7.5 Update `examples/ml_interactive.rs` to open a window and show the sigmoid plot texture
- [x] 7.6 Update README with quick start, example list, screenshots/GIFs, and dataset attribution
- [x] 7.7 Manually verify each spec scenario in the visualization specs against the running examples *(project owner reported a pass on 2026-10-02; no screenshots supplied)*
