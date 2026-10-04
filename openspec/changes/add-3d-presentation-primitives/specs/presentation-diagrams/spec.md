## ADDED Requirements

### Requirement: Declarative diagram model
The library SHALL provide a diagram model of nodes, edges, labels, and groups, each with a stable string id, and independent of Bevy rendering. A node SHALL have a role (input, bias, sum, activation, output), a label, a position, and a size. An edge SHALL connect two nodes, and MAY carry a weight, a label, and an arrowhead. A group SHALL name its member nodes. The model SHALL serialize with serde and SHALL be validated: ids unique, and every edge endpoint and group member SHALL exist.

#### Scenario: Valid diagram
- **WHEN** the built-in perceptron diagram is validated
- **THEN** validation succeeds and every id is unique

#### Scenario: Dangling edge
- **WHEN** a diagram has an edge whose `to` names no node
- **THEN** validation fails, naming the edge id and the missing node id

#### Scenario: Round trip
- **WHEN** a diagram is serialized to JSON and read back
- **THEN** the result equals the original

### Requirement: Perceptron diagram
The library SHALL build a perceptron diagram from three input weights and a bias:
- inputs x₁, x₂, x₃ and a bias node b;
- one weighted edge from each into a sum node Σ;
- an arrowed edge Σ → activation σ, and σ → output y;
- an input-layer group.

Edge labels SHALL show each weight's value.

#### Scenario: Structure
- **WHEN** the perceptron is built with weights (0.8, −0.5, 0.3) and bias 0.1
- **THEN** it has 7 nodes and 6 edges, the edges into Σ carry weights 0.8, −0.5, 0.3, and 0.1, and the edges Σ → σ and σ → y have arrowheads

### Requirement: Composed only from existing primitives
The 3-D rendering of a diagram SHALL build every mesh from Bevy's built-in primitives (`Sphere`, `Capsule3d`, `Cylinder`, `Cone`, and `Extrusion` of Bevy 2-D primitives) or from an adopted FOSS crate. It SHALL NOT contain bevaru code that generates vertices, tessellates outlines, or extrudes shapes. Bevaru code SHALL only choose primitives and place them with transforms. The rendering SHALL reuse one mesh per primitive kind, whatever the diagram's size.

#### Scenario: Shared meshes
- **WHEN** a perceptron and a diagram with twice as many nodes and edges are rendered in turn
- **THEN** both use the same number of mesh assets

#### Scenario: Provenance documented
- **WHEN** the presentation docs are read
- **THEN** every geometry operation the diagram uses names the Bevy type or crate that performs it

### Requirement: Weight encoding
An edge's weight SHALL be encoded by colour (one hue for positive, a contrasting hue for negative, readable with common colour-vision deficiencies) and by tube radius, which increases with |w| between a minimum and a maximum radius. A zero weight SHALL still draw at the minimum radius in a neutral colour. Tubes SHALL end at node surfaces, and arrowheads SHALL sit at the head node's surface.

#### Scenario: Sign and magnitude
- **WHEN** edges with weights 0.8 and −0.5 are rendered
- **THEN** they have different hues, and the 0.8 tube is thicker than the −0.5 tube

#### Scenario: Ends at the surface
- **WHEN** an edge joins two nodes of radius r
- **THEN** its tube starts and ends r from each node's centre

### Requirement: 2-D slide projection
The library SHALL project a diagram through a camera view into normalised slide coordinates ([0, 1]², origin top-left), giving every node's centre and apparent size, and every edge's endpoints and label position. The projection SHALL be a pure function, testable without a GPU.

#### Scenario: Front-on projection
- **WHEN** a diagram is projected from a camera looking straight at its front
- **THEN** the order of node positions left to right and top to bottom matches their world x and y, and all coordinates lie in [0, 1]

#### Scenario: Matches the camera
- **WHEN** a node is projected through the experience's home view
- **THEN** its slide position agrees, to within one pixel at 1920 × 1080, with Bevy's `Camera::world_to_viewport` for the same camera

### Requirement: Perceptron experience
The diagram SHALL be available as a lobby experience, "Perceptron in 3D" (id `perceptron`), in a *Diagrams* category, with an embedded thumbnail and an example, `examples/perceptron_3d.rs`, that opens it. It SHALL provide:
- an orbit, zoom, and reset camera;
- labels projected from the scene;
- controls for the three weights, the bias, the activation (step or sigmoid), and label visibility.

Changing a control SHALL update the diagram in place. Leaving SHALL leave nothing behind.

#### Scenario: Open from the lobby
- **WHEN** a user activates the "Perceptron in 3D" card
- **THEN** the perceptron is shown with its labels and controls

#### Scenario: Live weight
- **WHEN** a weight slider moves from positive to negative
- **THEN** that edge's colour and thickness and its label update without reloading the experience

#### Scenario: Clean exit
- **WHEN** the experience is entered and left ten times
- **THEN** entity, mesh, material, and image counts return to the lobby baseline

### Requirement: Slide view
The experience SHALL offer a slide view, toggled with `H`, that hides every control and the lobby button, keeps the diagram's labels, frames the diagram for a 16:9 slide on a white background, and leaves `Esc` returning to the lobby. A window capture taken in slide view SHALL be usable as a slide image.

#### Scenario: Toggle
- **WHEN** the user presses `H` in the experience and then `H` again
- **THEN** the controls and lobby button disappear with labels still shown, and then reappear

### Requirement: Presentation documentation
The project SHALL document, under `docs/presentation/`:
- **Geometry provenance:** which Bevy type or crate performs each geometry operation.
- **Upstream gap log:** each missing capability, the upstream project it belongs in, related existing upstream issues, and a draft issue ready to file. Nothing is filed without the maintainer's approval.
- **Shape vocabulary:** at least the issue's list of common presentation shapes (rectangle, rounded rectangle, ellipse, triangle, diamond, arrow, double arrow, curved arrow, chevron, star, cross, bracket, brace, callout, speech bubble, flowchart symbols, and straight, elbow, and curved connectors). Each is keyed by its ECMA-376 preset name, classified (fixed preset, adjust-parameterised, path-defined, or connector), and mapped to a route through the FOSS geometry stack, with gaps marked.
- **Licensing boundary:** Euro-Office is AGPL-3.0 and is a reference only; no Euro-Office code, data, or translated formulas enter bevaru; shape names and semantics come from ECMA-376; any reuse of the standard's definition formulas is an open question.
- **Path to slides:** static capture now, an embedded interactive (WASM) build later, and an editable 2-D fallback generated from the diagram model's projection as ECMA-376 presets.

The README SHALL describe the experience, link these documents, and credit [vgarciasc/simulated-annealing-viz](https://github.com/vgarciasc/simulated-annealing-viz) as an inspiration.

#### Scenario: Vocabulary covers the issue
- **WHEN** the shape-vocabulary document is read
- **THEN** each shape in the issue's list has an ECMA-376 name, a classification, and a route or a linked gap

#### Scenario: Licensing stated
- **WHEN** the licensing document is read
- **THEN** it states that no Euro-Office code or data is copied into bevaru, and names ECMA-376 as the source of shape names

#### Scenario: Credit
- **WHEN** the README is read
- **THEN** it credits vgarciasc/simulated-annealing-viz as an inspiration
