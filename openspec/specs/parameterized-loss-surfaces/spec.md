# Parameterized Loss Surfaces Specification

## Purpose

Define how the training objective is sampled over one model weight and the bias, rendered as an orbitable 3-D surface, and updated live as the loss and its hyperparameters change.
## Requirements
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

### Requirement: Lobby experience
The parameter-space objective surfaces SHALL also be a lobby experience, "Training objective in 3D", in the *Loss functions* category, with an embedded thumbnail. Its controls, camera, and live updates SHALL match the `loss_surface` example. The example SHALL open the experience directly, so the two cannot drift. Leaving the experience SHALL leave nothing behind.

#### Scenario: Open from the lobby
- **WHEN** a user activates the "Training objective in 3D" card
- **THEN** the hinge objective surface is shown with its loss selector, parameter sliders, and orbit controls

#### Scenario: Example and lobby agree
- **WHEN** `cargo run --example loss_surface` is started, and separately the card is chosen in the lobby
- **THEN** both start the same registered experience

#### Scenario: Clean exit
- **WHEN** the experience is entered and left ten times
- **THEN** entity, mesh, material, and image counts return to the lobby baseline

