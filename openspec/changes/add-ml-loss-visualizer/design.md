## Context

`bevaru` is a young repository whose stated purpose is to use [ruviz](https://github.com/Ameyanagi/ruviz) (a Rust plotting library) for machine-learning visualization in Bevy. It currently holds a single root crate pinned to Bevy 0.14 and ruviz 0.14.2: `BevaruPlugin` renders a sigmoid line plot to PNG bytes (`PlotPngBytes`), regenerated on `RefreshPlotEvent`. Nothing displays those bytes yet — the example runs under `MinimalPlugins` and logs the byte count. This change creates the first feature: interactive, animated visualization of loss functions and how they — and their hyperparameters — shape linear models (regression, SVM, logistic regression) on synthetic data, Iris, and MNIST.

Constraints:
- Everything trained must be small enough to step at interactive rates (≥ 60 FPS render, training steps between frames).
- The maths must be trustworthy: this is a teaching tool, and a visibly wrong gradient teaches the wrong thing.
- MNIST is ~50 MB raw and has licence/hosting quirks, so it cannot be bundled.

## Goals / Non-Goals

**Goals:**
- A reusable Bevy plugin that others can drop into an app, plus runnable examples that work out of the box.
- A Bevy-free core (losses, models, datasets) that is fully unit-tested, including gradient checks.
- Watching the hyperplane move: during training, during a hyperparameter sweep, and when switching losses.
- Iris and MNIST as first-class datasets alongside synthetic teaching cases.

**Non-Goals:**
- Kernel SVMs, neural networks, and non-linear decision boundaries (natural follow-ups, deliberately excluded here).
- A general ML library or competitive training performance. Solvers are chosen for visual clarity and determinism, not speed.
- GPU training, WASM/web builds (should not be precluded, but not targeted or tested in this change).
- Loss *landscape* surfaces over parameter space (a strong future candidate; see Open Questions).

## Decisions

### 1. Cargo workspace: root `bevaru` + `crates/bevaru-core`
`bevaru-core` holds losses, models, datasets, projection — no Bevy dependency. The existing root crate `bevaru` stays the Bevy plugin (keeping its published name and API) and depends on the core. The scaffold's `build_sigmoid_data` moves to core as the logistic function.
- **Why**: the maths is testable with plain `cargo test` in seconds, without a GPU or window; spec `loss-functions` requires Bevy independence. It also lets non-Bevy users (notebooks via PyO3, CLI tools) reuse the core later.
- **Alternative**: one crate with a `bevy` feature flag. Rejected — feature-gated modules tend to leak Bevy types into the core over time, and the boundary is harder to enforce.

### 2. Losses as an enum, not a trait object
`LossKind` enum with `value(x, &params)` / `grad(x, &params)` methods dispatching via `match`; hyperparameters live in a `LossParams` struct with validated constructors.
- **Why**: the set is closed and small; an enum gives exhaustive matching, trivial runtime switching from UI, `Copy`, and easy `Reflect` / serialization. Unit-testing every variant is a single loop over `LossKind::ALL`.
- **Alternative**: `trait Loss` with `Box<dyn Loss>`. More extensible for third parties but awkward to drive from a dropdown and to (de)serialize. Revisit if users need custom losses — a `Custom(Arc<dyn Loss>)` variant can be added without breaking the API.

### 3. One solver: (sub)gradient descent with explicit steps
All models train with full-batch or mini-batch (sub)gradient descent; SVMs use the primal objective `½‖w‖² + C · mean(hinge)`, with a Pegasos-style decaying step size option.
- **Why**: one uniform step function makes every model animatable the same way, and the primal hinge objective is exactly the "loss + regularizer" story the visualization tells. A dual/SMO solver would converge faster but its intermediate states (α vectors) don't map to a hyperplane that means anything mid-training.
- **Trade-off**: subgradient descent on hinge loss converges slowly and can jitter. Mitigated by decaying step size and, for sweeps, running to convergence off-thread (decision 6).

### 4. Linear algebra: `nalgebra` with `f64` in core, `f32` in rendering
- **Why**: `f64` keeps gradient checks and convergence tests stable; Bevy uses `f32`, so conversion happens once at the plugin boundary. `nalgebra` `DMatrix`/`DVector` handles both 2-D and 784-D cleanly.
- **Alternative**: `ndarray`. Comparable; `nalgebra` chosen for its first-class symmetric eigendecomposition (needed for PCA) and good `glam` interop.

### 5. Snapshot history in the trainer, interpolation in the plugin
The core `Trainer` stores `Vec<Snapshot>` (step, `w`, `b`, loss, support-vector indices). The plugin owns *display* state: a `DisplayedModel` component that eases toward a target snapshot.
- **Why**: scrubbing back costs nothing and is exact. Snapshots of 784-D models are ~6 KB each; capping history (default 10 000 steps, configurable, with decimation beyond) bounds memory.
- **Interpolation**: interpolate the boundary as *(unit normal, signed offset)* — slerp the normal, lerp the offset — not by lerping raw `w`, which can pass through `w ≈ 0` and make the line fly off to infinity (spec: "interpolation preserves orientation"). The scale `‖w‖` (margin width `1/‖w‖`) is interpolated geometrically, so margins change smoothly across a C sweep spanning orders of magnitude.
- **As built**: the displayed boundary approaches its target exponentially (`1 − e^{−4.6·dt/duration}` per frame, ~99% after `transition_secs`) rather than as a fixed-duration tween. During playback the target moves every frame; a tween restarted each frame from smoothstep's flat start would barely move. Exponential approach handles continuous playback and discrete jumps (scrubbing, sweep steps) alike.
- **As built**: the trainer caches `f(x)` for the current model and reuses it for the next gradient, and full-batch gradients are two matrix–vector products (`Xw`, `Xᵀg`). This was needed for MNIST: per-row loops made 784-D steps too slow to animate. Playback also caps training at 8 ms of wall-clock per frame so expensive models slow training rather than freezing the UI.

### 6. Sweeps precomputed on `AsyncComputeTaskPool`
A sweep spawns one task per sampled hyperparameter value, each training to convergence from the same seed and initialization; results stream back via a channel; the animation plays once enough values are ready.
- **Why**: keeps the render thread at frame rate (spec requirement), and warm-starting is avoided so each frame is the true solution for that value rather than path-dependent.
- **Alternative**: warm-start each value from the previous solution (faster, smoother). Offered as a sweep option later; default is cold-start for correctness.

### 7. Charts with ruviz; controls with `bevy_egui`
- **Charts**: loss curves and the training-loss plot are rendered by `ruviz` — the project's stated plotting backend and what the scaffold already uses — into a Bevy `Image`, shown as an `egui::Image` in the chart panel. **As built**: ruviz's `render()` returns straight-alpha RGBA directly, so there is no PNG encode/decode; a 3-series 640×400 chart renders in ~2 ms (release, warm). The `egui_plot` fallback was not needed. Each chart's inputs are hashed; a render starts only when the hash changes, at most every 100 ms, and a result whose inputs have since changed is discarded. The scaffold's `PlotPngBytes`/`RefreshPlotEvent` pattern generalizes to one render per chart, triggered by change detection on chart inputs. Rendering runs on `AsyncComputeTaskPool` so slider drags never block a frame; stale renders are dropped if a newer request exists.
- **Alternative**: `egui_plot`. Interactive hover and zero encode/decode cost, but sidesteps the project's purpose. Kept as a fallback if ruviz render latency proves too high for slider-drag rates (see Risks).
- **Controls**: sliders, dropdowns and timelines are what egui excels at; building them on Bevy UI would be most of the project.
- The scene itself (points, hyperplanes, regions) is rendered in Bevy proper, so it gets proper cameras, 3D, and later shaders.
- **Trade-off**: an extra dependency whose version must track Bevy's. Pin compatible versions together (`bevy_egui` 0.42 with Bevy 0.19, default features off to avoid pulling in `bevy_ui`).
- **As built**: egui gets its own full-window camera (no render layers, drawn last with alpha blending). Left to auto-attach, bevy_egui binds to the first pane camera and lays the whole UI out inside that pane's viewport.

### 8. Rendering approach
- **Points**: instanced meshes (one mesh per marker shape, per-instance colour). 70 000 MNIST points is excessive — the UI subsamples (default 2 000).
- **Hyperplane (2D)**: a line clipped to the visible data bounds; margins as dashed lines. **(3D)**: the exact polygon where the plane cuts the data's bounding box (3–6 vertices), as a translucent mesh; margins as dashed polygon outlines.
- **Decision regions**: **as built**, a 160 × 160 CPU-computed texture on a quad behind the points, recomputed only when the shown boundary changes (~26 k evaluations — negligible). This replaced the planned custom shader: same visual result, no WGSL tied to Bevy's shader imports, and unit-testable.
- **Panes**: **as built**, each comparison pane's content sits at its own world offset (10 000 units apart) with its own camera viewport, so panes need no render layers or per-pane gizmo groups. Up to three panes.
- **World frame**: display coordinates are mapped into roughly ±5 world units with one uniform scale, so angles and margin widths stay geometrically honest and marker sizes are dataset-independent.
- **Gizmos** are used for residual segments and support-vector rings (cheap, immediate-mode, rebuilt every frame).
- **Palette**: Okabe–Ito colours plus distinct marker shapes per class.

### 9. High-dimensional models and projections
For MNIST the model trains in 784-D; the 2D view shows the PCA projection of the data and the intersection of the true decision boundary with the PCA plane through the data mean, explicitly labelled "projection". Alongside it, `w` is shown as a 28×28 diverging-colormap image — the clearer picture of what a linear MNIST classifier learned.
- **Alternative**: train on the 2 PCA components only. Simpler and the line is exact, but accuracy collapses and the lesson becomes misleading. Offered as an option ("train in projected space").

### 10. MNIST acquisition
Behind the `mnist` feature: `ureq` (blocking, small) on a background task, SHA-256 verify with `sha2` against pinned hashes, cache in `dirs::cache_dir()/bevaru/mnist`. Primary source is a stable mirror; an env var `BEVARU_MNIST_DIR` points at a pre-downloaded copy for offline/CI use.
Iris (≈4 KB CSV, public domain via UCI) is embedded with `include_str!`.

### 11. Bevy version
The scaffold pins Bevy 0.14, which is several releases behind. Upgrade to the current stable Bevy as the first implementation task, before writing plugin code, and pin it and `bevy_egui` exactly; Bevy breaks APIs every minor release (e.g. `Event`/`EventReader` naming changed after 0.14). The scaffold's two tests are the regression check for that upgrade. If the upgrade proves blocked (e.g. a ruviz incompatibility), stay on 0.14 and pin the matching `bevy_egui`.
**As built**: Bevy `=0.19.1` (features `2d`, `3d`) with `bevy_egui` `=0.42.0`. Buffered events became `Message`s in this range, so `RefreshPlotEvent` now derives `Message` (name unchanged); both scaffold tests passed after the upgrade.

## Risks / Trade-offs

- [Subgradient descent on hinge loss jitters near convergence, which looks like a bug] → decaying step size by default; convergence uses a windowed loss change, not single-step deltas; document the jitter in the example as part of the lesson.
- [MNIST mirror disappears or changes] → pinned checksums fail loudly rather than training on wrong data; `BEVARU_MNIST_DIR` override; mirror URL list rather than a single URL.
- [ruviz render too slow for live slider drags] → resolved: raw RGBA output, ~2 ms per chart, rendered off-thread and throttled to 10/s.
- [Vulkan validation messages in debug builds] → `VUID-vkAcquireNextImageKHR-semaphore-01286` is logged by wgpu's swapchain handling on this machine (Mesa RADV). Seen only in debug runs and at swapchain acquire/screenshot time; rendering and screenshots are correct. Cause not yet investigated.
- [Bevy / bevy_egui version churn] → exact version pins, examples built in CI, upgrade as a deliberate change.
- [Projected boundary of a 784-D model can look nonsensical in 2D] → always labelled as a projection; weight-image view provided as the faithful representation; "train in projected space" option for the exact-line view.
- [Large `C` with fixed learning rate diverges] → learning rate scaled by `1/(1 + C)` by default for SVM sweeps; divergence (non-finite parameters) detected and surfaced in the UI instead of rendering NaNs.
- [Memory growth from snapshot history on long runs] → capped history with decimation.

## Migration Plan

Not applicable — greenfield repository. Rollback is reverting the change.

## Open Questions

- ~~Does ruviz offer a non-PNG output path?~~ Yes: `Plot::render()` returns straight-alpha RGBA (`ruviz::core::Image`). Used directly.
- Should a loss-landscape view (loss surface over `(w₁, w₂)` or over `(w, b)` for 1-D regression, with the optimizer's path drawn on it) be part of this change or the next? It fits the "watch parameters change" goal well and is cheap for 2-parameter models.
- Multi-class: is one-vs-rest on Iris (three hyperplanes at once) wanted in this change, or is binary sufficient for the first release?
- Licence/attribution text to display for MNIST and Iris in the UI.
