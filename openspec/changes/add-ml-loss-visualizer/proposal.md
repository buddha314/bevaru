## Why

Loss functions and their hyperparameters are usually taught with static plots, which hide the thing that matters: how the *fitted model* moves as a loss or hyperparameter changes. bevaru exists to make ML concepts interactive in Bevy, and loss functions are the natural first subject — small enough to compute in real time, foundational enough to be worth seeing, and directly connected to classic models (linear regression, SVM) on famous datasets (Iris, MNIST).

## What Changes

- Extend the existing `BevaruPlugin` scaffold (today: a ruviz-rendered sigmoid plot exposed as `PlotPngBytes`, refreshed by `RefreshPlotEvent`) into a set of focused sub-plugins that render ML concepts as interactive, animated scenes, and actually display ruviz plots on screen as textures.
- Add a Bevy-independent core of loss functions: **MSE, MAE, Huber, hinge, squared hinge, logistic (log) loss, and 0-1 loss**, each with value and (sub)gradient, plus their hyperparameters (Huber `δ`, hinge margin, regularization `λ` / SVM `C`).
- Add linear models trained step-by-step by gradient / subgradient descent: linear regression (any regression loss), linear soft-margin SVM (hinge / squared hinge), and logistic regression. Training is incremental so every step can be drawn.
- Add dataset support: **Iris** bundled in-crate; **MNIST** downloaded on demand, checksum-verified and cached; small synthetic generators (linearly separable, overlapping, outlier-contaminated regression) for clean teaching cases. Includes 2D projection (feature-pair selection, PCA).
- Add visualization primitives: data points coloured by class, the decision hyperplane (line in 2D, plane in 3D), SVM margin lines and highlighted support vectors, shaded decision regions, regression fit lines with residuals, a side-by-side loss-curve chart, and a live training-loss plot.
- Add animation and interaction: play / pause / step / scrub training; sweep any hyperparameter over a range and watch the hyperplane morph; switch loss function live; UI panel of sliders and toggles.
- Add runnable examples: `loss_curves`, `regression_mse_vs_mae` (outlier robustness), `iris_svm` (C sweep), `mnist_svm` (binary digit pair, weight vector shown as a 28×28 image).

## Capabilities

### New Capabilities
- `loss-functions`: Bevy-independent loss functions with values, (sub)gradients, and typed hyperparameters.
- `linear-models`: Incrementally trainable linear regression, linear SVM, and logistic regression, exposing per-step snapshots of parameters and loss.
- `datasets`: Iris (bundled), MNIST (on-demand download + cache), synthetic generators, and projection to 2D/3D for display.
- `visualization-scene`: The Bevy plugin and its rendering of points, hyperplanes, margins, support vectors, decision regions, residuals, and loss charts.
- `parameter-animation`: Playback of training, hyperparameter sweeps with interpolated hyperplane animation, and the interactive control panel.

### Modified Capabilities
<!-- none — repository has no existing specs -->

## Impact

- **Existing code**: the root `bevaru` crate (`src/lib.rs`, `examples/ml_interactive.rs`) stays the plugin crate; the repo becomes a Cargo workspace with a new Bevy-free `crates/bevaru-core` member. The existing `PlotPngBytes` / `RefreshPlotEvent` API is kept and generalized.
- **New code**: `bevaru-core` (math, models, datasets), plugin modules, and new `examples/`.
- **Dependencies**: existing `bevy` (currently pinned to 0.14) and `ruviz` (2D charts); new `bevy_egui` (control panel, version-matched to Bevy), a small linear-algebra crate (`nalgebra`), and — behind an `mnist` cargo feature — an HTTP client and a SHA-256 crate for download verification.
- **Network**: MNIST download is the only network access, opt-in via feature flag, and cached locally after first fetch.
- **Repository**: the scaffold from PR #1 is extended, not replaced; its public API and tests are preserved.
