## 1. Numeric Surface

- [x] 1.1 Add a validated, Bevy-independent objective-surface sampler over a chosen weight and bias.
- [x] 1.2 Test grid indexing, agreement with `objective`, invalid requests, and parameter-driven changes for hinge and logistic loss.

## 2. Visual Asset

- [x] 2.1 Convert sampled grids to indexed, coloured Bevy meshes with a fixed height scale.
- [x] 2.2 Add a `loss_surface` example with hinge, squared hinge, and logistic selection, applicable parameter sliders, and live mesh updates.
- [x] 2.3 Add orbit, zoom, frame reset, axis labels, and objective-range labels to the example.

## 3. Documentation and Verification

- [x] 3.1 Document the command and the surface's parameter/objective axes in the README.
- [x] 3.2 Run formatting, Clippy, core and example build checks, then visually inspect the surface and interactions.
