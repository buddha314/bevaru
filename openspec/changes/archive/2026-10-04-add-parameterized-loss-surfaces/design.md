## Context

`bevaru-core::model::objective` already computes the mean pointwise loss plus weight regularization. The existing loss chart samples one margin or residual, while the Bevy scene has orbit camera behavior and mesh rendering. Binary cross-entropy is represented by `LossKind::Logistic`.

## Goals / Non-Goals

**Goals:**

- Show the objective as a surface over one selected model weight and the bias for fixed training data.
- Make hinge, squared hinge, and logistic objectives comparable in the same coordinate system.
- Rebuild the mesh when a loss parameter or regularization strength changes; keep orbit and zoom interactive.
- Export sampling and mesh construction so later examples can reuse the asset.

**Non-Goals:**

- A full high-dimensional landscape explorer, GPU sampling, or optimizer path overlay.
- New pointwise loss formulas or changes to existing training behavior.

## Decisions

1. **Use model parameter coordinates.** The horizontal axes are a chosen weight and bias; height is the existing full-dataset objective. A single pointwise hinge or logistic loss has only one input, so this supplies a meaningful second axis for both. Other model weights stay at a caller-provided base value.
2. **Sample in `bevaru-core`.** A validated grid sampler calls the existing `objective` function, returning row-major values and bounds. This keeps the numeric asset independent of Bevy and straightforward to test.
3. **Build a Bevy mesh from the grid.** A reusable mesh builder emits indexed triangles with vertex colours keyed to objective value. The example owns the UI and camera, while library code owns the numeric-to-mesh conversion.
4. **Use a fixed vertical scale within a session.** Changing C, loss type, or hinge margin must visibly change geometry. The example labels the actual objective range and offers a height-scale control rather than normalizing each mesh independently.
5. **Start with a one-feature binary dataset.** The example uses deterministic data so its axes can be labelled `weight` and `bias`. The sampler itself supports any feature dimension and selected weight index.

## Risks / Trade-offs

- [Large regularization or wide parameter ranges can produce tall surfaces] → Bound controls and provide height scale and reset framing.
- [Synchronous resampling could stall a frame at high grid sizes] → Use a fixed modest grid for the first example and rebuild only on settings changes.
- [The objective depends on the dataset and fixed weights] → Label the surface and state the held-fixed values in the UI and README.
