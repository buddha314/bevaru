# Parameter Animation Specification

## Purpose

Define interactive training playback, sweeps, and comparison controls.

## Requirements

### Requirement: Training playback
The plugin SHALL let the user play, pause, single-step, and scrub training, and set playback speed in steps per second.

#### Scenario: Pause halts the model
- **WHEN** playback is paused
- **THEN** the trainer does not advance and the displayed hyperplane stays fixed

#### Scenario: Single step
- **WHEN** the user presses "step" while paused
- **THEN** exactly one training step is applied and displayed

#### Scenario: Scrub backwards
- **WHEN** the user drags the timeline to an earlier step
- **THEN** the scene displays that step's snapshot without re-running training

### Requirement: Hyperparameter sweep
The plugin SHALL animate a sweep of one hyperparameter (e.g. SVM `C`, regularization `λ`, Huber `δ`, hinge margin, learning rate) across a user-defined range, linear or logarithmic, showing the converged model for each value.

#### Scenario: C sweep on Iris
- **WHEN** a log sweep of `C` from `0.01` to `100` is run on a binary Iris task
- **THEN** the hyperplane and margins animate through the converged solution for each sampled `C`, and the current `C` is displayed

#### Scenario: Sweep values are precomputed
- **WHEN** a sweep is started
- **THEN** solutions are computed off the render thread and the animation plays at a steady frame rate once they are ready, with progress shown while computing

### Requirement: Smooth interpolation between states
When the displayed model changes between sweep values or between snapshots during playback, the rendered hyperplane SHALL interpolate smoothly over a configurable duration rather than jumping.

#### Scenario: Interpolation is visible
- **WHEN** the sweep advances from one `C` value to the next with a 0.3 s transition
- **THEN** the boundary passes through intermediate positions over 0.3 s

#### Scenario: Interpolation preserves orientation
- **WHEN** interpolating between two boundaries whose normals differ by angle
- **THEN** the boundary rotates through that angle rather than collapsing through `w = 0`

### Requirement: Live loss switching
The user SHALL be able to switch the active loss among those valid for the current task; the trainer SHALL restart from the same initialization with the new loss.

#### Scenario: MSE to MAE on outlier data
- **WHEN** the user switches from MSE to MAE on regression data with outliers
- **THEN** training restarts and the fit line converges closer to the inlier trend than the MSE fit did

### Requirement: Control panel
The plugin SHALL provide a UI panel with: dataset and binary-task selection, projection selection, loss and model selection, a slider per hyperparameter (with log scale where appropriate), playback controls, sweep configuration, and visibility toggles for margins, regions, residuals, and charts.

#### Scenario: Slider edits apply live
- **WHEN** the user drags the `λ` slider during playback
- **THEN** training continues from its current parameters with the new `λ`

#### Scenario: Only valid options shown
- **WHEN** the active dataset is a classification task
- **THEN** the loss selector offers only classification losses

### Requirement: Comparison mode
The plugin SHALL support running two configurations side by side on the same data (e.g. hinge vs logistic, MSE vs MAE, `C = 0.1` vs `C = 10`), with synchronized playback.

#### Scenario: Synchronized comparison
- **WHEN** two configurations are compared and playback advances 10 steps
- **THEN** both panes show step 10
