## Why

The current loss charts show one-dimensional slices. Issue #8 asks for loss geometry that users can orbit and change with parameters. A model-parameter objective surface makes hinge and binary cross-entropy comparable without inventing a second pointwise input for either loss.

## What Changes

- Add a reusable sampler for the objective over one model weight and the bias, keeping the dataset and other weights fixed.
- Render the sampled objective as an orbitable 3-D mesh with labelled parameter axes and a height legend.
- Add controls for hinge, squared hinge, and logistic (binary cross-entropy) loss, plus their applicable margin and regularization parameters; update the mesh when controls change.
- Add a runnable `loss_surface` example and explain the surface axes in the README.

## Capabilities

### New Capabilities

- `parameterized-loss-surfaces`: Sampling, rendering, and interacting with 3-D loss objective surfaces.

### Modified Capabilities

None. The existing pointwise losses, trainer objective, and visualization requirements remain valid.

## Impact

Adds a Bevy-independent sampler in `bevaru-core`, a reusable surface component and plugin in `bevaru`, one example, and documentation. Uses existing Bevy, egui, and core dependencies.
