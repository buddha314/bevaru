# loss-shapes Specification

## Purpose

Show each loss as a 3-D surface over two quantities students already know, such as truth and prediction, true and predicted probability, two class scores, a value and a hyperparameter, or three classes. The 2-D loss curve appears as a slice of each surface. Every value is sampled from the library's own loss functions.

## Requirements
### Requirement: Loss-shape catalogue
The library SHALL define a catalogue of 3-D loss-shape views, identified by stable kebab-case ids. It SHALL cover every loss in `LossKind::ALL` in at least one view, in five families:
- **prediction vs truth:** MSE, MAE, and Huber over true value y and prediction ŷ;
- **probability vs truth:** binary cross-entropy over true probability p and predicted probability q;
- **two class scores:** hinge, squared hinge, logistic (softmax cross-entropy), and 0-1 over the correct-class score and the other-class score;
- **hyperparameter as an axis:** Huber over residual r and δ; hinge and squared hinge over margin m and the margin parameter;
- **three classes:** cross-entropy over the probability simplex, and softmax cross-entropy, Weston–Watkins hinge, and Crammer–Singer hinge over two rival scores.

Each view SHALL declare:
- its axis names, units, and ranges;
- its height label;
- the hyperparameters it uses;
- a caption of at most about 80 words, saying what the axes are, where the loss is zero or minimal, and what the shape teaches.

#### Scenario: Every loss has a shape
- **WHEN** the catalogue is enumerated
- **THEN** every `LossKind` appears in at least one view, and every view has a unique id, two named axes with finite ranges, a height label, and a non-empty caption

### Requirement: Sampling matches the loss functions
Sampling a view SHALL produce a grid whose every value equals the corresponding loss function evaluated at that grid point, with the view's hyperparameters applied. Sampling SHALL be independent of Bevy.

#### Scenario: Prediction vs truth
- **WHEN** the MSE prediction-vs-truth view is sampled
- **THEN** the value at (y, ŷ) equals `LossKind::Mse.value(ŷ − y)`, and every point on the diagonal ŷ = y is 0

#### Scenario: Two class scores
- **WHEN** the hinge two-scores view is sampled with margin 1
- **THEN** the value at (z_correct, z_other) equals `max(0, 1 − (z_correct − z_other))`, and points along any line parallel to z_correct = z_other have equal values

#### Scenario: Logistic and binary cross-entropy agree
- **WHEN** the logistic two-scores view and the binary cross-entropy view are compared at true probability p = 1 and predicted probability q = σ(z_correct − z_other)
- **THEN** the two losses are equal to within 1e-9

#### Scenario: Hyperparameter axis
- **WHEN** the Huber residual-and-δ view is sampled
- **THEN** each row at a fixed δ equals the 2-D Huber curve for that δ

### Requirement: Binary cross-entropy surface
The probability-vs-truth view SHALL plot −[p ln q + (1 − p) ln(1 − q)] over p ∈ [0, 1] and q in [ε, 1 − ε], with a stated ε. Heights SHALL be clipped at a cap (default 8) that is labelled on the height axis, so the surface stays finite and readable. It SHALL offer an "entropy removed" mode that plots the KL divergence instead (cross-entropy minus H(p)), and it SHALL highlight the curve of minima q = p.

#### Scenario: Valley floor is the entropy
- **WHEN** the cross-entropy surface is sampled
- **THEN** along q = p the value equals H(p) = −[p ln p + (1 − p) ln(1 − p)], and it is the minimum of its row at fixed p

#### Scenario: KL mode
- **WHEN** entropy-removed mode is on
- **THEN** values along q = p are 0 and every value is ≥ 0

#### Scenario: Never infinite
- **WHEN** the surface is sampled at its q range ends with p = 0 or 1
- **THEN** every value is finite and at most the cap, and clipped points are marked as clipped

### Requirement: Three-class views
The library SHALL provide three-class loss functions: softmax cross-entropy, Weston–Watkins multiclass hinge (the sum over rivals of max(0, Δ + z_j − z_true)), and Crammer–Singer multiclass hinge (the largest such term). It SHALL show them in two views:
- **Probabilities on a triangle:** cross-entropy −ln q_true over the probability simplex q₁ + q₂ + q₃ = 1, drawn on an equilateral (ternary) base with the true class at a labelled corner, and clipped at the same cap as binary cross-entropy;
- **Two rival scores:** with the true class's score fixed at 0, each loss over the two rival scores (z₂, z₃).

#### Scenario: Only the true probability matters
- **WHEN** the probability-triangle view is sampled
- **THEN** any two points with the same q_true have equal loss, so contour lines run parallel to the edge opposite the true class

#### Scenario: Sum versus worst violation
- **WHEN** both hinge views are sampled with Δ = 1 at (z₂, z₃) = (0.5, 0.5)
- **THEN** Weston–Watkins gives 3 (1.5 + 1.5), Crammer–Singer gives 1.5, and softmax cross-entropy gives ln(1 + 2e^0.5)

#### Scenario: Agrees with the binary case
- **WHEN** the third class's score is very negative (z₃ = −50)
- **THEN** each three-class loss equals its binary two-scores counterpart at the same gap, to within 1e-9

#### Scenario: Triangle edges
- **WHEN** the probability-triangle view is rendered
- **THEN** its base is triangular (no samples outside the simplex), and its corners are labelled with the classes

### Requirement: Linked 2-D slice
Each view SHALL identify the slice of its surface that equals the existing 2-D loss curve for that loss, using the 2-D chart's own argument (residual, or margin). It SHALL draw that slice as a plane and as a highlighted curve on the surface. The 2-D loss-curve chart SHALL remain available alongside, unchanged.

#### Scenario: Slice equals the 2-D chart
- **WHEN** the hinge two-scores view shows its slice (z_other = 0)
- **THEN** the highlighted curve's values equal the 2-D hinge curve over the same margins

#### Scenario: 2-D charts kept
- **WHEN** the loss-shapes experience is open
- **THEN** the 2-D loss curve for the selected loss is shown beside the 3-D view

### Requirement: Probe and gradient
Pointing at the surface SHALL show:
- the two input values and the loss value at that point;
- an arrow along the negative (sub)gradient, pointing downhill.

At a kink, it SHALL say which subgradient is shown. Where the gradient is zero (a flat region, or 0-1 loss), it SHALL say there is no gradient rather than draw an arrow.

#### Scenario: Probe on MSE
- **WHEN** the probe is at (y = 0, ŷ = 2) on the MSE prediction-vs-truth view
- **THEN** it reports loss 4 and an arrow pointing toward the diagonal ŷ = y

#### Scenario: Flat region
- **WHEN** the probe is beyond the margin on the hinge view, or anywhere on the 0-1 view
- **THEN** it reports "no gradient here" and draws no arrow

### Requirement: 3-D rendering
Views SHALL render as shaded surfaces with:
- a cool-to-warm height colormap and a colour legend;
- tick-labelled axes;
- the clip level marked when values are capped;
- orbit, zoom, and a reset-view control.

Switching view or changing a hyperparameter SHALL update the surface in place, without reloading the experience.

#### Scenario: Live hyperparameter
- **WHEN** the Huber δ slider changes in a Huber view
- **THEN** the surface, the highlighted slice, and the 2-D curve update to the new δ within one frame of sampling completing

### Requirement: Loss-shapes experience
The shapes SHALL be available as a lobby experience, "Loss shapes in 3D", under the *Loss functions* category. It SHALL have:
- an embedded thumbnail;
- controls to pick the family, the loss, and the hyperparameters;
- an example, `examples/loss_shapes.rs`, that opens it directly.

Leaving it SHALL leave nothing behind, as for every experience.

#### Scenario: Open from the lobby
- **WHEN** a user activates the "Loss shapes in 3D" card from the *Loss functions* category
- **THEN** the binary cross-entropy surface is shown with its caption and controls

#### Scenario: Clean exit
- **WHEN** the experience is entered and left ten times
- **THEN** entity, mesh, material, and image counts return to the lobby baseline

