## Context

bevaru shows losses three ways today:
- **2-D loss curves:** each loss against its single argument, the residual r = ŷ − y or the margin m = y·f(x) (`charts::loss_curve_chart`).
- **3-D objective surfaces over model parameters** (#18): `bevaru-core::surface::sample_objective_surface`, `src/loss_surface.rs::objective_surface_mesh`, and the standalone `loss_surface` example.
- **Pointwise loss functions:** `LossKind::value` and `LossKind::grad` in `bevaru-core`.

What's missing is the shape of a loss **as a function of its inputs**, which is how losses are taught. The reference picture is the binary cross-entropy surface over true and predicted probability: high corners where they disagree, and a valley along the diagonal.

The audience is students, so every surface has to be *explainable*. Its axes must be quantities they already know, and its shape must make one idea obvious.

## Goals / Non-Goals

**Goals:**
- One or more explainable 3-D views for every loss bevaru has.
- Each 3-D view visibly contains the 2-D curve students already know, as a slice.
- Every sampled value is exactly the library's loss function: tested, not approximated.
- A lobby experience and an example; agents can sample the same views.

**Non-Goals:**
- Replacing or changing the 2-D charts, or #18's parameter-space surfaces.
- Surfaces over four or more classes. Three classes is the most that keeps "the shape is the idea": a probability triangle, or two rival scores.
- Training on these surfaces. They are of the loss function itself, not of a model's objective; that's #18's job.

## Decisions

### 1. Five families, chosen for what each shape teaches

| Family | Axes | Losses | The idea it makes visible |
| ------ | ---- | ------ | ------------------------- |
| `prediction-vs-truth` | true y, prediction ŷ ∈ [−3, 3]² | MSE, MAE, Huber | Loss is zero exactly on the diagonal ŷ = y, and depends only on the residual, so the trough has the same cross-section everywhere. MSE's trough is a parabola, MAE's a V with a crease (no gradient at zero), and Huber's a V with a rounded bottom of width δ. |
| `probability-vs-truth` | true p ∈ [0, 1], predicted q ∈ [ε, 1 − ε] | binary cross-entropy | Disagreement is punished without bound (hence the cap). The valley floor is the entropy H(p), which is not zero, so a confident model of a 50/50 truth still pays. Removing H(p) gives KL divergence, which is zero exactly when q = p. In short: cross-entropy = entropy + KL. |
| `two-scores` | correct-class score z_c, other-class score z_o ∈ [−4, 4]² | hinge, squared hinge, logistic, 0-1 | Classification losses depend only on the gap z_c − z_o (the margin), so each surface is a sheet folded along that direction. Hinge goes flat once the gap exceeds the margin; logistic keeps nudging forever; 0-1 is a cliff with no gradient anywhere. This is the binary case of the multiclass hinge and softmax cross-entropy in standard ML courses. |
| `hyperparameter` | (r, δ) for Huber; (m, μ) for hinge and squared hinge | Huber, hinge, squared hinge | The whole family of 2-D curves for a hyperparameter at once. Huber turns from MAE-like (small δ) to MSE-like (large δ) inside |r| ≤ δ; the hinge crease slides with the margin μ. |
| `three-class` (probabilities) | the simplex q₁ + q₂ + q₃ = 1, on a ternary base | 3-class cross-entropy | Loss is −ln q_true, so only the probability on the true class matters: contour lines run parallel to the opposite edge, and how the wrong classes split the remainder changes nothing. |
| `three-class` (scores) | rival scores z₂, z₃ ∈ [−4, 4]², true score fixed at 0 | softmax cross-entropy, Weston–Watkins hinge, Crammer–Singer hinge | How multiclass losses treat several rivals. Softmax is a smooth maximum: it mostly listens to the strongest rival. Weston–Watkins adds up every violation, so two rivals cost double. Crammer–Singer counts only the worst. On the same axes, the three surfaces differ exactly along the diagonal where both rivals compete. |

- **Ids:** views are an exhaustive `ShapeView` enum with kebab-case ids, e.g. `prediction-vs-truth-huber`, `probability-vs-truth-cross-entropy`, `two-scores-hinge`, `hyperparameter-huber-delta`.
- **Default view:** `probability-vs-truth-cross-entropy`, the reference picture.
- **Why these axes:** each pair is something students already reason about: the answer and my guess, the true and predicted probability, two class scores, a value and a knob. Alternatives considered:
  - *loss over a 2-D feature space* for a fixed classifier: a nice link to the SVM scene, but it shows a model as much as a loss;
  - *parameter-space surfaces*: #18 already does those.

### 2. Sampling in `bevaru-core::shapes`, exact by construction
- **Grid type:** `SurfaceGrid { x: Axis, y: Axis, values: Vec<f64>, clipped: Vec<bool>, height_label }`, where `Axis { name, unit, min, max, n }`. #18's `ObjectiveSurface` gains a `to_grid()`, so one mesh builder serves both.
- **Exact by construction:** `sample(view, &ShapeParams, resolution, mode) -> Result<SurfaceGrid, ShapeError>` evaluates the existing `LossKind::value` at each point, with no re-implementations:
  - prediction vs truth: `loss.value(ŷ − y)`;
  - two scores: `loss.value(z_c − z_o)`;
  - hyperparameter views: `value(r)` with δ, or μ, set per row.
- **Three-class losses:** `softmax_cross_entropy(scores, true)`, `hinge_weston_watkins(scores, true, Δ)`, and `hinge_crammer_singer(scores, true, Δ)`, as plain functions, not `LossKind`s, since the trainer is binary. Softmax uses log-sum-exp for stability. A test pins each to its binary counterpart when the third score is very negative.
- **Ternary base:** sampled on the right triangle (q₂, q₃) with q₁ = 1 − q₂ − q₃. Points outside the simplex are masked out of `SurfaceGrid` (`values: Vec<Option<f64>>`), and the mesh builder skips masked cells. An affine map draws the triangle equilateral, with class labels at the corners.
- **Binary cross-entropy:** a new function, `cross_entropy(p, q)`, evaluated stably with `ln_1p` where it helps. It is deliberately not a new `LossKind`: it is the same loss as `Logistic` in probability form (logistic(m) = −ln σ(m) = BCE(1, σ(m))), and a test pins that identity (spec: "Logistic and binary cross-entropy agree").
- **Odd resolution (default 81):** puts the diagonal and the zero lines exactly on samples, so the MAE crease and the hinge flat region are sharp.
- **The cap (default 8, as in the reference image):** applied to cross-entropy heights only; clipped points are flagged. The other families are finite on their ranges.

### 3. The 2-D slice is defined by the view, not drawn by hand
Each view declares its slice:
- `y = 0`, giving the residual axis, for prediction vs truth;
- `z_o = 0`, giving the margin axis, for two scores;
- `p = 1`, plotted against the margin m = ln(q / (1 − q)), for cross-entropy;
- the current hyperparameter row, for hyperparameter views.

The highlighted curve and the 2-D chart are both computed from that declaration, and a test compares the slice with `loss_curve_chart` data. The existing ruviz 2-D chart sits beside the 3-D view, unchanged.

### 4. Rendering: generalize #18's mesh, add a colormap and a probe
- **Mesh:** `src/loss_surface.rs` gains `grid_mesh(&SurfaceGrid, …)`, and `objective_surface_mesh` becomes a thin wrapper, so #18's API and behaviour are kept.
- **Colour:** vertex colours come from ruviz's `ColorMap::coolwarm()` (blue–white–red, the reference image's colormap) over [min, cap] heights, so the live view matches the rendered PNGs. It stays readable for common colour-vision deficiencies because height also encodes the value. There is a colour legend.
- **Scale:** the surface fits a fixed box (10 × 10 × 6 world units) with one scale for both input axes, so diagonals stay at 45° where the axes share units. The height axis has ticks in loss units.
- **Annotations:** axis tick labels and captions are egui overlays projected from world positions, like the scene's pane labels. The clip level is a translucent plane with a label.
- **Probe:** Bevy's mesh picking gives the hit point; the probe recomputes the loss and gradient from the core functions at the hit's input coordinates, not from interpolated vertices. The arrow shows −∇L, projected onto the surface. Where the subgradient is ambiguous (the MAE crease, the hinge corner), the label names the choice the library uses (0 at the MAE kink, −1 below the hinge margin).
- **Camera:** an orbit, zoom, and reset rig. The `loss_surface` example's rig is moved into a small shared module both use.

### 5. Custom experiences, not experiments
The loss-shapes experience has no dataset or trainer, so it registers as `ExperienceKind::Custom`:
- **Cleanup:** its entities carry `ExperienceEntity`, and its resources are removed on `ExperienceStopped`, so it inherits the lobby's leak guarantees.
- **Systems:** gated with `in_experience("loss-shapes")`.
- **Layout:** controls on the left (family, loss, hyperparameters, KL toggle, slice toggle, resolution); the caption and the 2-D chart on the right.
- **Re-sampling:** when parameters change, it re-samples off the main thread (an 81² grid takes milliseconds) and swaps in the mesh.
- **#18 in the lobby, "Training objective in 3D":** its example's setup, controls, and systems move into `src/experiences/loss_surface.rs` as a second custom experience in the same category. Its camera rig is the shared module from decision 4. `examples/loss_surface.rs` becomes `shared::run_experience("loss-surface")`, so the example and the lobby can't drift. The two cards side by side tell one story: the shape of the loss, then the shape of training with it.

### 6. Agents
- **Manifest:** a `loss_shapes` section, built from `ShapeView::ALL` through an exhaustive match, so a new view without a description fails to compile. It lists each view's id, family, losses, axes and ranges, hyperparameters, slice, and caption.
- **Tool:** `sample_loss_shape`, in the `losses` group. Its input is `LossShapeRequest { view, huber_delta?, margin?, resolution?, entropy_removed? }`; it returns the grid, the clipped mask, the caption, and the slice, with resolution capped at 101.
- **Rendered images:** `render_loss_shape`, in the `charts` group, renders the same grid with ruviz's 3-D `surface` (the `3d` feature, which only adds `base64`, already in the tree). It uses `ColorMap::coolwarm()`, labelled axes, and an optional azimuth and elevation. The interactive mesh samples the same ruviz colormap for its vertex colours, so the live view and the agent's PNG look alike. Masked (off-simplex) samples become gaps in the ruviz surface. If ruviz can't leave gaps, the triangle view is rendered on its right-triangle base and labelled as such.
- **Docs:** the generated `docs/agents/capabilities.*` pick this up automatically.

## Risks / Trade-offs

- **[A cell-wide ramp, not a true cliff]** The 0-1 "cliff" and the hinge corner are sampled on a grid. → Odd resolution aligns the edges with samples; the 0-1 caption says the wall is vertical; a resolution slider lets students sharpen it.
- **[Unbounded cross-entropy]** Cross-entropy grows without bound near q ∈ {0, 1}. → Clip at a labelled cap, flag clipped points, and state ε in the axis label.
- **[Too much on screen]** A surface plus a slice, probe, legend, and 2-D chart is a lot. → Each overlay has a toggle; defaults are surface, legend, slice, and caption.
- **[Losses in one view have different height ranges]** Hinge is linear, squared hinge quadratic. → The z axis rescales per loss, with tick labels in loss units; switching loss is a visible rescale, not a silent one.
- **[Mesh picking may not be in bevaru's Bevy features]** → If the picking backend isn't available, march the ray over the heightfield on the CPU. That's cheap at 81², and it works with the same probe code.

## Open Questions

None blocking. Resolved with the maintainer on 2026-10-04: add the three-class views, render PNGs for agents, and bring #18's surfaces into the lobby.
