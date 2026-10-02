## ADDED Requirements

### Requirement: Bevy plugin entry point
The crate SHALL expose `BevaruPlugin`, which adds all visualization systems, and SHALL also expose its constituent sub-plugins individually so an app can opt into only what it needs. The existing `PlotPngBytes` resource and `RefreshPlotEvent` event SHALL remain available.

#### Scenario: Headless use still works
- **WHEN** an app adds only `MinimalPlugins` and `BevaruPlugin`
- **THEN** startup succeeds and `PlotPngBytes` is populated, as with the existing scaffold

#### Scenario: Minimal app
- **WHEN** an app adds `DefaultPlugins` and `BevaruPlugin` and spawns a scene from a dataset and model config
- **THEN** the scene renders with no further setup

### Requirement: Data points
The scene SHALL render each sample as a marker positioned by its projection, coloured by class (classification) or uniformly (regression), using a colour-blind-safe palette and a distinct shape per class so class is not conveyed by colour alone.

#### Scenario: Iris classes are distinguishable
- **WHEN** Iris is rendered
- **THEN** the three classes use three different colours and three different marker shapes, and a legend names them

### Requirement: Decision hyperplane
The scene SHALL render the model's decision boundary `w·x + b = 0`: a line for 2D displays and a plane for 3D displays, updated every frame the model parameters change.

#### Scenario: Boundary tracks parameters
- **WHEN** the model's `w` or `b` changes
- **THEN** the rendered boundary moves to the new position in the same frame

#### Scenario: Degenerate weights
- **WHEN** `w` is the zero vector
- **THEN** no boundary is drawn and the UI shows "no boundary (w = 0)" instead of panicking or drawing at infinity

### Requirement: SVM margins and support vectors
For SVM models the scene SHALL render the margin lines `w·x + b = ±1` and visually highlight current support vectors.

#### Scenario: Support vectors highlighted
- **WHEN** an SVM snapshot lists support vectors
- **THEN** exactly those points are rendered with a highlight ring

### Requirement: Decision regions
For classifiers the scene SHALL optionally shade the plane by predicted class, with intensity proportional to confidence (`|f(x)|`, or probability for logistic regression).

#### Scenario: Toggle regions
- **WHEN** the user disables decision regions
- **THEN** the shading is hidden and points and boundary remain visible

### Requirement: Regression fit and residuals
For regression models the scene SHALL render the fitted line and optionally a vertical residual segment from each point to the fit.

#### Scenario: Outlier residuals visible
- **WHEN** a regression dataset with outliers is shown with residuals enabled
- **THEN** each point has a residual segment whose length equals `|ŷ − y|` in display units

### Requirement: ruviz charts displayed in-scene
2D charts (loss curves, training loss) SHALL be rendered with `ruviz` and displayed in the Bevy window as textures, re-rendered only when their inputs change.

#### Scenario: Chart updates on input change
- **WHEN** a chart's inputs change (hyperparameter, selected losses, new training step)
- **THEN** the chart is re-rendered and its on-screen texture replaced within one frame of the render completing

#### Scenario: No redundant renders
- **WHEN** no chart inputs change for 60 frames
- **THEN** no ruviz render occurs during those frames

### Requirement: Loss curve chart
The plugin SHALL render a chart of loss value against residual (regression) or margin (classification) for one or more selected losses overlaid, with the current hyperparameters applied.

#### Scenario: Comparing classification losses
- **WHEN** hinge, logistic, and 0-1 loss are selected
- **THEN** all three are drawn on shared axes over margin `m ∈ [−3, 3]` with a legend

#### Scenario: Chart reflects hyperparameter
- **WHEN** the Huber `δ` slider changes
- **THEN** the Huber curve redraws with the new transition point

### Requirement: Training loss plot
The plugin SHALL render a live plot of training loss versus step for the active trainer, with a marker at the currently displayed step.

#### Scenario: Scrub marker
- **WHEN** playback is scrubbed to step 40
- **THEN** the training-loss plot marker sits at step 40

### Requirement: Camera
2D scenes SHALL support pan and zoom; 3D scenes SHALL support orbit, pan, and zoom. A "frame data" action SHALL fit all points in view.

#### Scenario: Frame data
- **WHEN** the user triggers "frame data"
- **THEN** every sample is within the viewport
