## 1. Sampling (bevaru-core)

- [x] 1.1 Add `SurfaceGrid` and `Axis`, and `ObjectiveSurface::to_grid()`, keeping #18's API
- [x] 1.2 Add `cross_entropy(p, q)` and `entropy(p)`, numerically stable, with a test pinning logistic(m) = BCE(1, σ(m))
- [x] 1.3 Add `ShapeView` (exhaustive, kebab-case ids, `ALL`), `ShapeFamily`, `ShapeParams`, and per-view metadata: axes, ranges, height label, hyperparameters, slice, and caption
- [x] 1.4 Add the three-class losses (softmax cross-entropy via log-sum-exp, Weston–Watkins and Crammer–Singer hinges), and masked samples in `SurfaceGrid` for the simplex
- [x] 1.5 Implement `sample(view, params, resolution, mode)` for all five families via the existing `LossKind::value`, with the cross-entropy cap, a clipped mask, and entropy-removed (KL) mode
- [x] 1.6 Tests for the spec scenarios, including the three-class ones (only q_true matters; sum vs worst violation at (0.5, 0.5); agreement with the binary views when z₃ = −50): MSE diagonal is zero; hinge depends only on the gap; logistic = BCE; Huber rows equal 2-D curves; the cross-entropy valley is H(p); KL is ≥ 0 and zero on the diagonal; no infinities; every loss is covered

## 2. Rendering (bevaru)

- [x] 2.1 Generalize `src/loss_surface.rs` to `grid_mesh(&SurfaceGrid, …)`, with `objective_surface_mesh` as a wrapper; colour with ruviz's `ColorMap::coolwarm()`; fixed-box scaling; skip masked cells; the equilateral map for the ternary base, with corner labels
- [x] 2.2 Move the `loss_surface` example's orbit, zoom, and reset rig into a shared module, used by both
- [x] 2.3 Axis tick labels, the colour legend, and the clip plane as projected egui overlays
- [x] 2.4 The slice plane and highlighted curve from each view's declared slice; a test that slice values equal `loss_curve_chart` data
- [x] 2.5 Probe: hit point from mesh picking (or a heightfield ray march); loss and gradient from the core functions; the arrow, the subgradient note, and "no gradient here"

## 3. Experience

- [x] 3.1 Register the "Loss shapes in 3D" custom experience under *Loss functions*: controls panel, caption, the 2-D chart beside it, and off-thread re-sampling on change
- [x] 3.2 `examples/loss_shapes.rs` (via `shared::run_experience`); a thumbnail via `scripts/thumbnails.sh loss-shapes`
- [x] 3.3 Move #18's `loss_surface` example into `src/experiences/loss_surface.rs` as "Training objective in 3D"; reduce the example to `shared::run_experience("loss-surface")`; generate its thumbnail
- [x] 3.4 Add both experiences to the headless lobby tests, including the ten-round-trip leak test

## 4. Agents

- [x] 4.1 Add the `loss_shapes` manifest section from `ShapeView::ALL` (exhaustive), and the `LossShapeRequest` wire type with its schema
- [x] 4.2 Add the `sample_loss_shape` tool to the catalog, `agent::run`, and the `bevaru-mcp` dispatch; regenerate `docs/agents/capabilities.*`
- [x] 4.3 Enable ruviz's `3d` feature; verify its `surface` API (colormap, camera angles, gaps for masked samples); add `render_loss_shape` to the catalog, `agent::run`, and the MCP dispatch
- [x] 4.4 Tests: one manifest entry per view; the sampling tool's cross-entropy grid has the H(p) diagonal; an unknown view lists valid ids; the rendering tool returns a PNG headlessly

## 5. Docs and verification

- [x] 5.1 README: a "Loss shapes in 3D" section with a screenshot, linked from the lobby section
- [x] 5.2 Check that it compiles with `scripts/check.sh`; run the test suites once with `scripts/check.sh --tests` before merging (not on every push)
- [x] 5.3 Look at every view in a real window: shapes, captions, slice, probe, legend, and resolution changes
