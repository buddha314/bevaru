## ADDED Requirements

### Requirement: Linear model family
The core crate SHALL provide a linear model `f(x) = w·x + b` trainable as: linear regression (with any regression loss), linear soft-margin SVM (hinge or squared hinge), and logistic regression (logistic loss). The model SHALL work for any input dimensionality.

#### Scenario: Pairing a model with an incompatible loss
- **WHEN** a classifier trainer is configured with MSE
- **THEN** configuration returns an error stating that MSE is a regression loss

#### Scenario: High-dimensional input
- **WHEN** a linear SVM is trained on 784-dimensional MNIST vectors
- **THEN** training succeeds and `w` has 784 components

### Requirement: SVM regularization via C
The SVM trainer SHALL expose the soft-margin parameter `C`, minimizing `½‖w‖² + C · Σ hinge(yᵢ f(xᵢ))` (mean-normalized), so that larger `C` penalizes margin violations more heavily.

#### Scenario: Large C narrows the margin on overlapping data
- **WHEN** the same overlapping two-class dataset is trained to convergence with `C = 0.01` and with `C = 100`
- **THEN** the `C = 100` model has a larger `‖w‖` (narrower geometric margin `2/‖w‖`) and a total training hinge loss no greater than the `C = 0.01` model's

### Requirement: Incremental training
Trainers SHALL advance one optimization step (full-batch or mini-batch gradient / subgradient descent) per call, so the renderer can draw intermediate states. Learning rate, batch size, step budget, and random seed SHALL be configurable.

#### Scenario: Stepping is deterministic
- **WHEN** two trainers with the same data, configuration, and seed are stepped 100 times
- **THEN** their parameters are bitwise identical after every step

#### Scenario: Loss decreases on a separable problem
- **WHEN** logistic regression is trained on a linearly separable synthetic dataset for 500 steps
- **THEN** the training loss after step 500 is lower than after step 1

### Requirement: Parameter snapshots
Each step SHALL produce a snapshot containing step index, `w`, `b`, mean training loss, and — for SVMs — the indices of support vectors (points with `y f(x) ≤ 1 + ε`). A trainer SHALL retain its snapshot history so playback can scrub backwards.

#### Scenario: Scrubbing to an earlier step
- **WHEN** a trainer has run 200 steps and snapshot 50 is requested
- **THEN** the returned `w`, `b` and loss equal those produced at step 50

#### Scenario: Support vectors reported
- **WHEN** an SVM snapshot is taken on a separable dataset after convergence
- **THEN** its support-vector set is non-empty and every listed point lies on or inside the margin

### Requirement: Convergence detection
Trainers SHALL report convergence when the relative change in loss falls below a configurable tolerance for a configurable number of consecutive steps, and SHALL stop advancing when converged or when the step budget is exhausted.

#### Scenario: Budget exhausted
- **WHEN** a trainer with a step budget of 10 is stepped 15 times
- **THEN** it reports status `BudgetExhausted` and its snapshot history has length 10
