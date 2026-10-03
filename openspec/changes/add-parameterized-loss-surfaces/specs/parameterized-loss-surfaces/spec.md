## ADDED Requirements

### Requirement: Objective surface sampling
The system SHALL sample the existing training objective on a rectangular grid over one chosen model weight and the bias, with other weights held fixed.

#### Scenario: Hinge and logistic values
- **WHEN** the same binary dataset and parameter grid are sampled with hinge and logistic losses
- **THEN** each grid value equals the existing `objective` function evaluated at that grid's weight and bias

#### Scenario: Invalid surface request
- **WHEN** a request has an invalid range, resolution, feature index, or incompatible dataset shape
- **THEN** sampling returns a descriptive error without producing a mesh

### Requirement: Reusable 3-D asset
The system SHALL turn a sampled objective grid into an indexed, coloured Bevy mesh whose horizontal coordinates represent the chosen weight and bias and whose height represents objective value at a fixed scale.

#### Scenario: Geometry follows objective
- **WHEN** a sampled grid is converted to a mesh
- **THEN** its vertices and triangles preserve the grid coordinates and objective heights

### Requirement: Interactive loss comparison
The system SHALL provide a runnable example that displays hinge, squared hinge, and logistic binary cross-entropy objective surfaces with orbit and zoom controls.

#### Scenario: Loss selection
- **WHEN** the user changes the selected loss
- **THEN** the surface is rebuilt for that loss without restarting the app

#### Scenario: Parameter edit
- **WHEN** the user changes C or the hinge margin for an applicable loss
- **THEN** the surface geometry and displayed objective range update to match the new objective

#### Scenario: Camera orbit
- **WHEN** the user drags or scrolls over the 3-D view
- **THEN** the camera orbits or zooms while the surface stays in the same parameter coordinates

### Requirement: Surface explanation
The example and README SHALL identify the two parameter axes, objective height, data dependence, and the logistic loss's binary cross-entropy interpretation.

#### Scenario: First launch
- **WHEN** a user opens the loss surface example
- **THEN** they can identify the axes and available controls from the on-screen UI and README command
