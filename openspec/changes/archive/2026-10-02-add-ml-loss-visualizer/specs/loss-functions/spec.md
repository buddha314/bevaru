## ADDED Requirements

### Requirement: Loss function catalogue
The core crate SHALL provide the following loss functions: mean squared error (MSE), mean absolute error (MAE), Huber, hinge, squared hinge, logistic (log) loss, and 0-1 loss. Each SHALL be identified by a variant of a single `LossKind` enum so the active loss can be switched at runtime.

#### Scenario: All losses are enumerable
- **WHEN** a caller iterates `LossKind::ALL`
- **THEN** it receives exactly the seven losses listed above, each with a human-readable display name

#### Scenario: Losses are classified by task
- **WHEN** a caller asks a `LossKind` for its task
- **THEN** MSE, MAE and Huber report `Regression`, and hinge, squared hinge, logistic and 0-1 report `Classification`

### Requirement: Pointwise value and gradient
Each loss SHALL expose a pointwise value and a pointwise (sub)gradient with respect to the model output. Regression losses SHALL be expressed in terms of the residual `r = ŷ − y`. Classification losses SHALL be expressed in terms of the margin `m = y · f(x)` with labels `y ∈ {−1, +1}`.

#### Scenario: MSE value and gradient
- **WHEN** MSE is evaluated at residual `r = 2`
- **THEN** the value is `4` and the gradient is `4` (using `L = r²`, `dL/dr = 2r`)

#### Scenario: MAE subgradient at zero
- **WHEN** MAE is evaluated at residual `r = 0`
- **THEN** the value is `0` and the subgradient is `0`

#### Scenario: Huber switches regime at delta
- **WHEN** Huber with `δ = 1` is evaluated at `r = 0.5` and at `r = 3`
- **THEN** the values are `0.125` (quadratic regime, `½r²`) and `2.5` (linear regime, `δ(|r| − ½δ)`)

#### Scenario: Hinge is zero beyond the margin
- **WHEN** hinge loss with margin `1` is evaluated at `m = 1.5` and at `m = 0.25`
- **THEN** the values are `0` and `0.75`, and the subgradients are `0` and `−1`

#### Scenario: Logistic loss is smooth
- **WHEN** logistic loss is evaluated at `m = 0`
- **THEN** the value is `ln 2` and the gradient is `−0.5`

#### Scenario: Gradients agree with finite differences
- **WHEN** any differentiable loss is evaluated at a point away from its kinks
- **THEN** its analytic gradient matches a central finite difference to within `1e-4` relative error

### Requirement: Typed loss hyperparameters
Loss hyperparameters SHALL be typed and validated: Huber `δ > 0`, hinge / squared-hinge margin `> 0`. Invalid values SHALL be rejected with an error rather than silently clamped.

#### Scenario: Invalid Huber delta rejected
- **WHEN** a Huber loss is constructed with `δ = 0`
- **THEN** construction returns an error naming the parameter and its valid range

### Requirement: Batch loss over a dataset
The core crate SHALL compute the mean loss over a dataset for given model parameters, optionally adding an L2 regularization term `λ/2 · ‖w‖²` that excludes the bias.

#### Scenario: Regularization excludes bias
- **WHEN** batch loss is computed with `λ = 1`, `w = [0, 0]`, bias `b = 5`, on data with zero loss
- **THEN** the total loss is `0`

### Requirement: Bevy independence
The loss-function module SHALL NOT depend on Bevy, so it can be unit-tested and reused without a renderer.

#### Scenario: Core builds without Bevy
- **WHEN** the core crate is built in isolation
- **THEN** `bevy` does not appear in its dependency tree
