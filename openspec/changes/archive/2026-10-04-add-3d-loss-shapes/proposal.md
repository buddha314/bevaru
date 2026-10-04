## Why

Students meet loss functions as 2-D curves ([#24](https://github.com/buddha314/bevaru/issues/24)). Many of the ideas behind them are about *two* quantities, though:
- **truth and prediction:** a loss is zero exactly where they agree;
- **two class scores:** classification losses only care about the gap between scores;
- **a value and a hyperparameter:** Huber's δ reshapes the curve.

A 3-D surface over those two quantities makes each idea visible as a shape: a trough along the diagonal, a fold that goes flat past the margin, a cliff with no gradient. The binary cross-entropy surface over true and predicted probability is the classic example.

bevaru already plots losses in 2-D, and since #18 it renders 3-D *objective* surfaces over model parameters. What it can't show is the shape of a loss *as a function of its inputs*. This change adds that, keeps the 2-D charts, and links each 2-D curve to the 3-D surface it is a slice of.

## What Changes

- **Five families of explainable 3-D loss shapes**, each with fixed, labelled axes and a one-paragraph caption saying what the shape teaches:
  - **Prediction vs truth** (MSE, MAE, Huber): loss over true value y and prediction ŷ. A trough along ŷ = y whose cross-section is the familiar curve.
  - **Probability vs truth** (binary cross-entropy): loss over true probability p and predicted probability q. Heights are clipped at a labelled cap, and a toggle subtracts the entropy H(p), turning cross-entropy into KL divergence.
  - **Two class scores** (hinge, squared hinge, logistic/softmax cross-entropy, 0-1): loss over the correct-class and other-class scores. Each is a folded sheet that depends only on the gap between them.
  - **Hyperparameter as an axis** (Huber over residual and δ; hinge and squared hinge over margin and their margin parameter): the whole family of 2-D curves as one surface.
  - **Three classes**, in two views:
    - **Probabilities on a triangle:** cross-entropy over every possible 3-class prediction, drawn on a ternary (triangle) base. Only the probability on the true class matters.
    - **Two rival scores:** softmax cross-entropy, Weston–Watkins hinge (sum of violations), and Crammer–Singer hinge (largest violation) over two rival scores, with the true class's score fixed. The difference between the three is visible.
- **Linked 2-D slice:** each view shows the slice plane that yields the existing 2-D loss curve, and highlights that curve on the surface. The 2-D charts are unchanged.
- **Probe:** pointing at the surface shows the inputs, the loss value, and the (sub)gradient as an arrow.
- **Rendering:**
  - a cool-to-warm height colormap;
  - axis ticks and labels;
  - a marked clip level where values are capped;
  - orbit, zoom, and reset.

  It reuses #18's surface mesh, generalized from parameter grids to any sampled grid.
- **Delivery:** a new "Loss shapes in 3D" lobby experience under *Loss functions*, with an embedded thumbnail and a `loss_shapes` example.
- **Bevy-independent sampling:** in `bevaru-core`, tested against the existing loss functions; binary cross-entropy is added as a probability-space function beside the margin-space logistic loss.
- **For agents:** the capability manifest lists every view. The MCP server gains two tools: one samples a view as a grid, the other renders it as a 3-D PNG using ruviz's `3d` feature. That feature adds only `base64`, which bevaru already uses.
- **#18 in the lobby:** #18's parameter-space objective surfaces become a lobby experience too, "Training objective in 3D", beside "Loss shapes in 3D". Its `loss_surface` example becomes a one-line wrapper, as the other examples are.

## Capabilities

### New Capabilities
- `loss-shapes`: The 3-D loss-shape views: the five families, their sampling, rendering, captions, linked 2-D slices, probe, and the lobby experience.

### Modified Capabilities
- `capability-manifest`: The manifest also describes every loss-shape view (family, losses, axes, ranges, and hyperparameters).
- `mcp-server`: Adds headless tools to sample a loss-shape view as a grid and to render it as a PNG.
- `parameterized-loss-surfaces`: #18's objective surfaces are also offered as a lobby experience.

## Impact

- **Code:**
  - new `bevaru-core::shapes` (grids, sampling, binary cross-entropy, and the 3-class losses: softmax cross-entropy and the Weston–Watkins and Crammer–Singer hinges);
  - `src/loss_surface.rs` generalized to any sampled grid, keeping #18's API;
  - new `src/experiences/loss_shapes.rs` and `src/experiences/loss_surface.rs` (custom experiences), `examples/loss_shapes.rs`, and `examples/loss_surface.rs` slimmed to a wrapper;
  - `src/agent/` (manifest section, wire type, tool, and `run` entry);
  - `crates/bevaru-mcp` (dispatch).
- **Docs:**
  - generated `docs/agents/capabilities.*`;
  - a README section and screenshot;
  - a lobby thumbnail.
- **Dependencies:** none new. ruviz gains its `3d` feature, which only adds `base64`, already in the tree.
- **Existing behaviour:** unchanged. The 2-D loss charts, #18's parameter surfaces, and existing ids stay as they are.
